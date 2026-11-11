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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::AppHandle;
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
