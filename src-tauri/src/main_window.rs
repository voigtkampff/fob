//! The main window's size, position and maximized state, kept across
//! restarts by tauri-plugin-window-state in `main-window.json` next to the
//! settings file. Only the main window is tracked: the mini window and the
//! status bar place themselves (companion.rs, overlay.rs), and whether the
//! main window opens at start stays with the start-in-tray setting.
//!
//! The plugin restores the window before it is first shown and saves when
//! the app quits. A resize or move is also saved once the window settles,
//! so a Windows shutdown or sign-out with Fob still running in the tray
//! keeps the last size too. While the window is maximized or minimized the
//! plugin keeps the normal size and position it had before.
//!
//! When Windows' display scale changes while Fob runs, tao resizes the window
//! and wry resizes the page inside it, but the page could stay drawn wrong
//! until the user dragged the window edges. `fit_webview` does once what that
//! drag does: it gives the page the window's scale and full client area and
//! has it lay out again. It runs on each scale change and when the window is
//! shown at a scale it was not fitted to (hidden in the tray during the
//! change), never on a timer.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, WebviewWindow};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

pub const LABEL: &str = "main";
const FILE: &str = "main-window.json";
const SETTLE: Duration = Duration::from_millis(500);

pub fn flags() -> StateFlags {
    StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED
}

pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_window_state::Builder::new()
        .with_filename(FILE)
        .with_state_flags(flags())
        .with_filter(|label| label == LABEL)
        .build()
}

/// Runs `save` once, after pokes have stopped for `after` (a drag or a
/// resize is many events).
pub struct Settle {
    last: Mutex<Option<Instant>>,
    waiting: AtomicBool,
    after: Duration,
}

impl Settle {
    pub const fn new(after: Duration) -> Self {
        Self {
            last: Mutex::new(None),
            waiting: AtomicBool::new(false),
            after,
        }
    }

    pub fn poke(&'static self, save: impl FnOnce() + Send + 'static) {
        if let Ok(mut last) = self.last.lock() {
            *last = Some(Instant::now());
        }
        if self.waiting.swap(true, Ordering::SeqCst) {
            return;
        }
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(self.after / 5);
                let still = self
                    .last
                    .lock()
                    .map(|l| l.is_none_or(|t| t.elapsed() >= self.after))
                    .unwrap_or(true);
                if still {
                    break;
                }
            }
            self.waiting.store(false, Ordering::SeqCst);
            save();
        });
    }
}

static SAVE: Settle = Settle::new(SETTLE);

/// The main window was resized or moved.
pub fn changed(app: &AppHandle) {
    let app = app.clone();
    SAVE.poke(move || {
        // On the main thread, like the plugin's own saves: reading the window
        // from here while holding its cache could wait on an event handler
        // that waits on the cache.
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = handle.save_window_state(flags());
        });
    });
}

/// Why the page is being fitted to the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Windows moved the window to another DPI.
    ScaleChanged,
    /// The window is being brought back; fit only if the DPI moved meanwhile.
    Shown,
}

/// The DPI the main page was last fitted at (0: never).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
static FITTED_DPI: AtomicU32 = AtomicU32::new(0);

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn needs_fit(fitted: u32, dpi: u32, why: Fit) -> bool {
    dpi != 0 && (why == Fit::ScaleChanged || fitted != dpi)
}

/// Queues the fit on the main thread, after the scale change has finished
/// resizing the window.
pub fn fit_webview(window: &WebviewWindow, why: Fit) {
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = window.hwnd() {
        let hwnd = hwnd.0 as isize;
        let _ = window.with_webview(move |webview| unsafe {
            fit(hwnd, &webview.controller(), why);
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, why);
}

#[cfg(target_os = "windows")]
unsafe fn fit(
    hwnd: isize,
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    why: Fit,
) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller3;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
    };
    use windows_0_58::Win32::Foundation as wv;
    use windows_core_0_58::Interface;

    let window = HWND(hwnd as _);
    let dpi = GetDpiForWindow(window);
    if !needs_fit(FITTED_DPI.load(Ordering::SeqCst), dpi, why) {
        return;
    }
    let mut client = RECT::default();
    if GetClientRect(window, &mut client).is_err() {
        return;
    }
    let (width, height) = (client.right - client.left, client.bottom - client.top);
    if width <= 1 || height <= 1 {
        return; // minimized: fitted when shown again
    }
    if let Ok(scaled) = controller.cast::<ICoreWebView2Controller3>() {
        let _ = scaled.SetRasterizationScale(f64::from(dpi) / 96.0);
    }
    // wry's own container window, resized now rather than asynchronously.
    let mut container = wv::HWND::default();
    if controller.ParentWindow(&mut container).is_ok() && !container.is_invalid() {
        let _ = SetWindowPos(
            HWND(container.0 as _),
            None,
            0,
            0,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
        );
    }
    // One pixel short and back, like the end of a manual resize, so the page
    // lays out again at the new scale even when its size did not change.
    for bottom in [height - 1, height] {
        let _ = controller.SetBounds(wv::RECT {
            left: 0,
            top: 0,
            right: width,
            bottom,
        });
    }
    let _ = controller.NotifyParentWindowPositionChanged();
    FITTED_DPI.store(dpi, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn size_position_and_maximized_are_kept_but_not_visibility() {
        let f = flags();
        assert!(f.contains(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED));
        // Start in tray decides whether the window shows; the title bar is ours.
        assert!(
            !f.intersects(StateFlags::VISIBLE | StateFlags::DECORATIONS | StateFlags::FULLSCREEN)
        );
    }

    #[test]
    fn only_the_main_window_is_tracked() {
        let main =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).unwrap();
        assert!(main.contains(".plugin(main_window::plugin())"));
        let here = include_str!("main_window.rs");
        assert!(here.contains(".with_filter(|label| label == LABEL)"));
        assert_eq!(LABEL, "main");
        let companion: String = include_str!("companion.rs").split_whitespace().collect();
        assert!(companion.contains(
            "(\"main\",WindowEvent::Resized(_)|WindowEvent::Moved(_))=>{crate::main_window::changed(app)"
        ));
    }

    #[test]
    fn the_page_is_fitted_on_each_scale_change_and_on_show_only_after_one() {
        assert!(needs_fit(0, 96, Fit::Shown), "never fitted");
        assert!(
            !needs_fit(144, 144, Fit::Shown),
            "shown again at the same scale"
        );
        assert!(needs_fit(96, 144, Fit::Shown), "scale changed while hidden");
        assert!(needs_fit(144, 144, Fit::ScaleChanged), "every scale change");
        assert!(needs_fit(144, 96, Fit::ScaleChanged), "and back");
        assert!(!needs_fit(96, 0, Fit::ScaleChanged), "no DPI, no window");
        let companion: String = include_str!("companion.rs").split_whitespace().collect();
        assert!(companion.contains(
            "crate::main_window::fit_webview(&main,crate::main_window::Fit::ScaleChanged)"
        ));
        assert!(companion
            .contains("crate::main_window::fit_webview(&window,crate::main_window::Fit::Shown)"));
        let here = include_str!("main_window.rs");
        let fit = &here[here.find("unsafe fn fit(").unwrap()..here.find("#[cfg(test)]").unwrap()];
        for persistent in ["save_window_state", "set_size", "fs::", "spawn", "sleep"] {
            assert!(!fit.contains(persistent), "{persistent}");
        }
    }

    #[test]
    fn a_burst_of_resizes_is_saved_once_after_it_settles() {
        static S: Settle = Settle::new(Duration::from_millis(400));
        static SAVES: AtomicUsize = AtomicUsize::new(0);
        let start = Instant::now();
        for _ in 0..20 {
            S.poke(|| {
                SAVES.fetch_add(1, Ordering::SeqCst);
            });
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(SAVES.load(Ordering::SeqCst), 0, "saved mid resize");
        let until = Instant::now() + Duration::from_secs(3);
        while SAVES.load(Ordering::SeqCst) == 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(SAVES.load(Ordering::SeqCst), 1);
        assert!(start.elapsed() >= Duration::from_millis(19 * 20 + 400));
        // A later resize is saved again.
        S.poke(|| {
            SAVES.fetch_add(1, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(1000));
        assert_eq!(SAVES.load(Ordering::SeqCst), 2);
    }
}
