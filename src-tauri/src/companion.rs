//! Native tray lifetime and a separate, rounded overview webview.
use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Rect, WindowEvent};

const TRAY_ID: &str = "neohtop-overview";
#[derive(Default)]
pub struct CompanionState {
    pub keep_running: bool,
    pub hidden_at: Option<Instant>,
    /// Fob: pinned mini window (behavior.miniPinned): focus loss doesn't hide it.
    pub pinned: bool,
    /// winperf: last tray icon rect (physical px), so resizes stay anchored.
    pub anchor: Option<(f64, f64, f64, f64)>,
    /// winperf: tray menu "Start in tray only" item, kept in sync with settings.
    pub start_item: Option<CheckMenuItem<tauri::Wry>>,
    /// winperf: tray items whose text follows the UI language.
    pub items: Option<TrayItems>,
    /// Fob: where the user dragged the pinned mini window (its visible
    /// rect, physical px), saved in mini-position.json. None: at the tray.
    user_rect: Option<PxRect>,
    /// Fob: the visible rect `place` last gave the window and when, so its
    /// own moves are not taken for a drag.
    placed: Option<(PxRect, Instant)>,
    /// Fob: the latest dragged rect and when it moved; saved once the
    /// window has been still for a moment (`saving`: a saver is waiting).
    moved: Option<(PxRect, Instant)>,
    saving: bool,
}

pub struct TrayItems {
    overview: MenuItem<tauri::Wry>,
    open: MenuItem<tauri::Wry>,
    start: CheckMenuItem<tauri::Wry>,
    language: Submenu<tauri::Wry>,
    /// Auto / English / 中文, in that order.
    languages: [CheckMenuItem<tauri::Wry>; 3],
    quit: MenuItem<tauri::Wry>,
}

const LANGUAGES: [&str; 3] = ["auto", "en", "zh"];

/// Set the tray texts for `pref` ("auto" | "en" | "zh") and tick it.
fn relabel(app: &AppHandle, pref: &str) {
    let locale = crate::i18n::resolve(Some(pref), crate::i18n::system_locale().as_deref());
    let text = crate::i18n::tray(locale);
    let service = app.state::<Mutex<CompanionState>>();
    let Ok(state) = service.lock() else {
        return;
    };
    let Some(items) = &state.items else { return };
    let _ = items.overview.set_text(text.show_mini);
    let _ = items.open.set_text(text.open_full);
    let _ = items.start.set_text(text.start_in_tray);
    let _ = items.language.set_text(text.language);
    let _ = items.languages[0].set_text(text.auto);
    let _ = items.quit.set_text(text.quit);
    for (item, id) in items.languages.iter().zip(LANGUAGES) {
        let _ = item.set_checked(id == pref);
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text.tooltip));
    }
}

/// A language was picked in the tray: save it (the UI and the tray follow).
fn set_language(app: &AppHandle, pref: &'static str) {
    relabel(app, pref);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let patch = serde_json::json!({ "behavior": { "language": pref } });
        let _ = crate::preferences::apply_patch(&app, &patch);
    });
}

/// Fob mini window: ONE fixed logical size for every view, set only when the
/// window is shown from the tray. Nothing resizes or moves it afterwards (no
/// view change, data refresh or click). The height is the sum of the fixed
/// blocks in src/lib/utils/mini-layout.js (MINI_HEIGHT there; a test checks).
const MINI_WIDTH: f64 = 360.0;
const MINI_HEIGHT: f64 = 652.0;

/// Physical size at a monitor scale. Round up: a pixel lost to rounding at
/// 125 %/150 % would cut the bottom row.
fn mini_size(scale: f64) -> tauri::PhysicalSize<u32> {
    tauri::PhysicalSize::new(
        (MINI_WIDTH * scale).ceil() as u32,
        (MINI_HEIGHT * scale).ceil() as u32,
    )
}

// Fob's tray icon: the hand-drawn 16/24/32 px frames of icons/icon.ico
// (design/fob-icon), so Windows does not have to downscale the large art.
const TRAY_FRAMES: [(u32, &[u8]); 3] = [
    (16, include_bytes!("../icons/tray-16.png")),
    (24, include_bytes!("../icons/tray-24.png")),
    (32, include_bytes!("../icons/tray-32.png")),
];

/// Smallest frame that covers the small-icon size at this display scale
/// (16 px at 100 %, 24 px at 150 %, 32 px at 200 %), else the largest.
fn tray_frame_size(scale: f64) -> u32 {
    let want = (16.0
        * if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        })
    .round() as u32;
    TRAY_FRAMES
        .iter()
        .map(|(size, _)| *size)
        .find(|size| *size >= want)
        .unwrap_or(32)
}

fn tray_icon(app: &AppHandle) -> tauri::Result<tauri::image::Image<'static>> {
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map_or(1.0, |m| m.scale_factor());
    let size = tray_frame_size(scale);
    let bytes = TRAY_FRAMES
        .iter()
        .find(|(s, _)| *s == size)
        .map_or(TRAY_FRAMES[2].1, |(_, b)| *b);
    Ok(tauri::image::Image::from_bytes(bytes)?.to_owned())
}

fn configure(
    app: &AppHandle,
    enabled: bool,
    keep_running: bool,
    start_in_tray: Option<bool>,
    language: Option<&str>,
) -> Result<(), String> {
    // Called only on the main thread: native menus and tray destruction require it.
    if enabled && app.tray_by_id(TRAY_ID).is_none() {
        let lang = |id: &str, text: &str| {
            CheckMenuItem::with_id(app, format!("lang-{id}"), text, true, false, None::<&str>)
                .map_err(|e| e.to_string())
        };
        let languages = [
            lang("auto", "Automatic (Windows)")?,
            lang("en", "English")?,
            lang("zh", "中文")?,
        ];
        let language = Submenu::with_items(
            app,
            "Language",
            true,
            &[
                &languages[0],
                &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                &languages[1],
                &languages[2],
            ],
        )
        .map_err(|e| e.to_string())?;
        let overview = MenuItem::with_id(app, "overview", "Show mini window", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let open = MenuItem::with_id(app, "open-main", "Open full window", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let start = CheckMenuItem::with_id(
            app,
            "start-in-tray",
            "Start in tray only (mini window mode)",
            true,
            start_in_tray.unwrap_or(true),
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;
        let line = || PredefinedMenuItem::separator(app).map_err(|e| e.to_string());
        let (first, second) = (line()?, line()?);
        let menu = Menu::with_items(
            app,
            &[&overview, &open, &first, &start, &language, &second, &quit],
        )
        .map_err(|e| e.to_string())?;
        if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
            state.start_item = Some(start.clone());
            state.items = Some(TrayItems {
                overview,
                open,
                start,
                language,
                languages,
                quit,
            });
        }
        TrayIconBuilder::with_id(TRAY_ID)
            .icon(tray_icon(app).map_err(|e| e.to_string())?)
            .tooltip("Fob — system overview")
            .menu(&menu)
            .menu_on_left_click(false)
            .on_menu_event(|app, event| {
                let _ = match event.id.as_ref() {
                    "overview" => show(app, None),
                    "open-main" => open_main(app),
                    "start-in-tray" => {
                        set_start_in_tray(app);
                        Ok(())
                    }
                    "quit" => {
                        app.exit(0);
                        Ok(())
                    }
                    "lang-auto" => {
                        set_language(app, "auto");
                        Ok(())
                    }
                    "lang-en" => {
                        set_language(app, "en");
                        Ok(())
                    }
                    "lang-zh" => {
                        set_language(app, "zh");
                        Ok(())
                    }
                    _ => Ok(()),
                };
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    rect,
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let app = tray.app_handle();
                    let Some(window) = app.get_webview_window("overview") else {
                        return;
                    };
                    if window.is_visible().unwrap_or(false) {
                        hide(app);
                    } else {
                        let recent = app
                            .state::<Mutex<CompanionState>>()
                            .lock()
                            .ok()
                            .and_then(|s| s.hidden_at)
                            .is_some_and(|t| t.elapsed() < Duration::from_millis(200));
                        if !recent {
                            let _ = show(app, Some(rect));
                        }
                    }
                }
            })
            .build(app)
            .map_err(|e| e.to_string())?;
    } else if !enabled {
        drop(app.remove_tray_by_id(TRAY_ID));
        if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
            state.items = None;
            state.start_item = None;
        }
    }
    if enabled {
        relabel(app, language.unwrap_or("auto"));
    }
    if let (Some(checked), Ok(state)) = (start_in_tray, app.state::<Mutex<CompanionState>>().lock())
    {
        if let Some(item) = &state.start_item {
            let _ = item.set_checked(checked);
        }
    }
    // Only opt in to hiding the main window when the tray actually exists.
    app.state::<Mutex<CompanionState>>()
        .lock()
        .map_err(|e| e.to_string())?
        // winperf: with a tray icon, closing the main window always hides it
        // to the tray (Quit is in the tray menu); the old preference is ignored.
        .keep_running = enabled;
    let _ = keep_running;
    Ok(())
}

/// The tray check item toggled itself; save its new state as the setting.
fn set_start_in_tray(app: &AppHandle) {
    let item = app
        .state::<Mutex<CompanionState>>()
        .lock()
        .ok()
        .and_then(|s| s.start_item.clone());
    let Some(item) = item else { return };
    let checked = item.is_checked().unwrap_or(true);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let patch = serde_json::json!({ "behavior": { "startInTray": checked } });
        if crate::preferences::apply_patch(&app, &patch).is_err() {
            let _ = item.set_checked(!checked); // settings not ready: undo
        }
    });
}

/// winperf launch: with "Start in tray only" (default) only the tray icon
/// appears, except on the first launch (see `shows_main_at_launch`).
/// Otherwise the main window opens as before.
pub fn launch(app: &AppHandle) -> Result<(), String> {
    let startup = crate::preferences::startup(app);
    // Fob: the mini window hotkey works in tray-only mode too.
    crate::hotkey::register_saved(app, &startup.mini_hotkey);
    let saved = load_position(app);
    if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
        state.pinned = startup.mini_pinned;
        state.user_rect = saved;
    }
    let tray_only = startup.tray
        && startup.start_in_tray
        && configure(app, true, true, Some(true), startup.language.as_deref()).is_ok();
    if shows_main_at_launch(tray_only, startup.first_launch) {
        let window = app
            .get_webview_window("main")
            .ok_or("Main window unavailable")?;
        return window.show().map_err(|e| e.to_string());
    }
    // Debug builds: WINPERF_SHOW_MINI=1 pops it every time (screenshots).
    if cfg!(debug_assertions) && std::env::var_os("WINPERF_SHOW_MINI").is_some() {
        show(app, None)?;
    }
    Ok(())
}
/// Fob: the main window opens at launch without tray-only mode (or without
/// a tray), and on the very first launch (no settings file yet) so the user
/// sees Fob started; later launches in tray-only mode show only the icon.
fn shows_main_at_launch(tray_only: bool, first_launch: bool) -> bool {
    !tray_only || first_launch
}

/// Title bar "Collapse to tray": hide the main window (minimize without a tray).
#[tauri::command]
pub async fn collapse_to_tray(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window unavailable")?;
    if app.tray_by_id(TRAY_ID).is_some() {
        window.hide().map_err(|e| e.to_string())
    } else {
        window.minimize().map_err(|e| e.to_string())
    }
}

/// winperf: Windows 11 rounds the opaque, frameless mini window and gives it
/// the native shadow, which follows the rounded corners (unlike a CSS shadow,
/// which the rectangular window would clip). Windows 10 ignores the
/// attribute: square corners with the native shadow.
pub fn native_frame(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    if let Some(window) = app.get_webview_window("overview") {
        if let Ok(h) = window.hwnd() {
            crate::ui::apply_mini_shape(h.0 as isize, window.scale_factor().unwrap_or(1.0));
        }
    }
    #[cfg(target_os = "windows")]
    if let Some(window) = app.get_webview_window("overview") {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        };
        if let Ok(hwnd) = window.hwnd() {
            // Fob v0.6.5: Windows' round corners are off; the squircle
            // region (ui::apply_mini_shape) shapes the window instead.
            let preference = DWMWCP_DONOTROUND;
            unsafe {
                let _ = DwmSetWindowAttribute(
                    HWND(hwnd.0 as _),
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &preference as *const _ as *const std::ffi::c_void,
                    std::mem::size_of_val(&preference) as u32,
                );
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

/// Fob: the global hotkey: show the mini window at the tray if it's hidden,
/// hide it if it's shown (pinned or not, like the tray icon).
pub fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window("overview") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide(app);
        return;
    }
    // Like the tray click: pressing the combo can take focus from the mini
    // window, which then hides itself just before this runs; don't reopen it.
    let recent = app
        .state::<Mutex<CompanionState>>()
        .lock()
        .ok()
        .and_then(|s| s.hidden_at)
        .is_some_and(|t| t.elapsed() < Duration::from_millis(200));
    if !recent {
        let _ = show(app, None);
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overview") {
        let _ = window.hide();
    }
    if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
        state.hidden_at = Some(Instant::now());
    }
    let _ = app.emit_to("overview", "companion-visibility", false);
}

/// A rectangle in physical pixels (virtual-screen coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
struct PxRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
impl PxRect {
    fn new(left: i32, top: i32, width: u32, height: u32) -> Self {
        Self {
            left,
            top,
            right: left.saturating_add(width as i32),
            bottom: top.saturating_add(height as i32),
        }
    }
    fn intersects(&self, o: &PxRect) -> bool {
        self.left < o.right && o.left < self.right && self.top < o.bottom && o.top < self.bottom
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}
/// The display the tray icon is on: its full rect, its work area (without the
/// taskbar) and, when known, the taskbar's edge, rect and auto-hide state.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Screen {
    monitor: PxRect,
    work: PxRect,
    taskbar: Option<(Edge, PxRect, bool)>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placed {
    pos: PhysicalPosition<i32>,
    /// The panel size: the requested one, or smaller when the work area
    /// (minus gaps) is smaller than that.
    size: (u32, u32),
    /// false: the requested panel was larger than the work area, so `size`
    /// was shrunk to keep it off the taskbar.
    fits: bool,
}
impl Placed {
    fn rect(&self) -> PxRect {
        PxRect::new(self.pos.x, self.pos.y, self.size.0, self.size.1)
    }
}

/// Fob: where the mini window goes. All geometry is physical pixels. The
/// panel sits inside the WORK AREA (never over the taskbar), a 12 px (logical)
/// gap from the taskbar edge like Windows 11 flyouts, centred on the tray icon
/// along the taskbar and clamped into the work area. An auto-hidden taskbar
/// does not shrink the work area, so its shown thickness is kept free.
fn placement(
    screen: &Screen,
    panel: (u32, u32),
    anchor: Option<(f64, f64, f64, f64)>,
    scale: f64,
) -> Placed {
    let (m, mut area) = (screen.monitor, screen.work);
    let edge = match screen.taskbar {
        Some((edge, bar, autohide)) if bar.intersects(&m) => {
            if autohide && area == m {
                match edge {
                    Edge::Bottom => area.bottom = area.bottom.min(bar.top.max(m.top)),
                    Edge::Top => area.top = area.top.max(bar.bottom.min(m.bottom)),
                    Edge::Left => area.left = area.left.max(bar.right.min(m.right)),
                    Edge::Right => area.right = area.right.min(bar.left.max(m.left)),
                }
            }
            edge
        }
        _ if area.bottom < m.bottom => Edge::Bottom,
        _ if area.top > m.top => Edge::Top,
        _ if area.left > m.left => Edge::Left,
        _ if area.right < m.right => Edge::Right,
        // No taskbar reserved (auto-hide we could not ask about, or macOS's
        // menu bar inside the monitor rect): the edge nearest the icon.
        _ => match anchor {
            Some((ax, ay, aw, ah)) => {
                let (cx, cy) = (ax + aw / 2.0, ay + ah / 2.0);
                let d = [
                    (Edge::Bottom, m.bottom as f64 - cy),
                    (Edge::Top, cy - m.top as f64),
                    (Edge::Left, cx - m.left as f64),
                    (Edge::Right, m.right as f64 - cx),
                ];
                d.iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0
            }
            None => Edge::Bottom,
        },
    };
    let (pw, ph) = (panel.0 as i32, panel.1 as i32);
    let gap = (12.0 * scale).round() as i32;
    let center = anchor.map(|(ax, ay, aw, ah)| (ax + aw / 2.0, ay + ah / 2.0));
    let (x, y) = match edge {
        Edge::Bottom | Edge::Top => (
            center.map_or(area.right - gap - pw, |(cx, _)| {
                (cx - pw as f64 / 2.0).round() as i32
            }),
            if edge == Edge::Bottom {
                area.bottom - gap - ph
            } else {
                area.top + gap
            },
        ),
        Edge::Left | Edge::Right => (
            if edge == Edge::Left {
                area.left + gap
            } else {
                area.right - gap - pw
            },
            center.map_or(area.bottom - gap - ph, |(_, cy)| {
                (cy - ph as f64 / 2.0).round() as i32
            }),
        ),
    };
    let fits_x = pw + 2 * gap <= area.right - area.left;
    let fits_y = ph + 2 * gap <= area.bottom - area.top;
    // Larger than the work area: shrink to it (never over the taskbar).
    let pw = pw.min(area.right - area.left - 2 * gap).max(1);
    let ph = ph.min(area.bottom - area.top - 2 * gap).max(1);
    let clamp = |v: i32, lo: i32, hi: i32| v.clamp(lo, hi.max(lo));
    Placed {
        pos: PhysicalPosition::new(
            clamp(x, area.left + gap, area.right - gap - pw),
            clamp(y, area.top + gap, area.bottom - gap - ph),
        ),
        size: (pw as u32, ph as u32),
        fits: fits_x && fits_y,
    }
}

/// The window rect that makes the window's VISIBLE rect `planned`, given
/// the window's current rect and visible rect (DWM frame bounds): invisible
/// resize borders or a shadow margin, if any, are added around `planned`.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn window_rect_for(planned: PxRect, window: PxRect, visible: PxRect) -> PxRect {
    PxRect {
        left: planned.left - (visible.left - window.left),
        top: planned.top - (visible.top - window.top),
        right: planned.right + (window.right - visible.right),
        bottom: planned.bottom + (window.bottom - visible.bottom),
    }
}
fn contains(outer: &PxRect, inner: &PxRect) -> bool {
    inner.left >= outer.left
        && inner.top >= outer.top
        && inner.right <= outer.right
        && inner.bottom <= outer.bottom
}

/// Windows: sets the window's rect directly (physical px) and reads back
/// what it really got. Tauri's set_size is not usable here: for an
/// undecorated window with a shadow, tao's set_inner_size computes the new
/// size from the CURRENT window rect and keeps it unchanged, and the window
/// was created larger than asked (AdjustWindowRectEx adds the WS_CAPTION
/// title bar and frame that WM_NCCALCSIZE then turns into client area).
/// Returns (window rect, visible rect) after placing.
/// Windows: (window rect, visible rect = DWM extended frame bounds).
#[cfg(target_os = "windows")]
fn window_rects(window: &tauri::WebviewWindow) -> Result<(PxRect, PxRect), String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let hwnd = HWND(window.hwnd().map_err(|e| e.to_string())?.0 as _);
    let px = |r: RECT| PxRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    };
    unsafe {
        let mut rc = RECT::default();
        GetWindowRect(hwnd, &mut rc).map_err(|e| e.to_string())?;
        let mut frame = RECT::default();
        let visible = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame as *mut RECT as *mut std::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .ok()
        .map(|_| px(frame))
        .filter(|f| f.right > f.left && f.bottom > f.top)
        .unwrap_or(px(rc));
        Ok((px(rc), visible))
    }
}
/// Windows: sets the window's rect directly (see the comment above).
#[cfg(target_os = "windows")]
fn set_window_rect(
    window: &tauri::WebviewWindow,
    planned: PxRect,
) -> Result<(PxRect, PxRect), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };
    let hwnd = HWND(window.hwnd().map_err(|e| e.to_string())?.0 as _);
    let rects = || window_rects(window);
    let set = |r: PxRect, flags| unsafe {
        SetWindowPos(
            hwnd,
            None,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            SWP_NOZORDER | SWP_NOACTIVATE | flags,
        )
        .map_err(|e| e.to_string())
    };
    // Move first, so a change of monitor (and DPI) happens before the size
    // is set; then the exact rect.
    set(planned, SWP_NOSIZE)?;
    set(planned, Default::default())?;
    let (win, visible) = rects()?;
    if visible != planned {
        // Invisible borders or a size Windows adjusted: correct once from
        // what the window really has.
        set(window_rect_for(planned, win, visible), Default::default())?;
        return rects();
    }
    Ok((win, visible))
}

/// The monitor containing `point` (the tray icon), its work area and the taskbar.
#[cfg(target_os = "windows")]
fn screen_at(point: (f64, f64), fallback: PxRect) -> Screen {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::Shell::{
        SHAppBarMessage, ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETSTATE, ABM_GETTASKBARPOS,
        ABS_AUTOHIDE, APPBARDATA,
    };
    let px = |r: RECT| PxRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    };
    let (monitor, work) = unsafe {
        let hmon = MonitorFromPoint(
            POINT {
                x: point.0.round() as i32,
                y: point.1.round() as i32,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut info).as_bool() {
            (px(info.rcMonitor), px(info.rcWork))
        } else {
            (fallback, fallback)
        }
    };
    let taskbar = unsafe {
        let mut data = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            ..Default::default()
        };
        if SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) != 0 {
            let autohide = SHAppBarMessage(ABM_GETSTATE, &mut data) as u32 & ABS_AUTOHIDE != 0;
            let edge = match data.uEdge {
                ABE_TOP => Edge::Top,
                ABE_LEFT => Edge::Left,
                ABE_RIGHT => Edge::Right,
                ABE_BOTTOM | _ => Edge::Bottom,
            };
            Some((edge, px(data.rc), autohide))
        } else {
            None
        }
    };
    Screen {
        monitor,
        work,
        taskbar,
    }
}
#[cfg(not(target_os = "windows"))]
fn screen_at(_point: (f64, f64), monitor: PxRect) -> Screen {
    Screen {
        monitor,
        work: monitor,
        taskbar: None,
    }
}

fn show(app: &AppHandle, rect: Option<Rect>) -> Result<(), String> {
    let window = app
        .get_webview_window("overview")
        .ok_or("Overview window unavailable")?;
    let rect = rect.or_else(|| {
        app.tray_by_id(TRAY_ID)
            .and_then(|tray| tray.rect().ok().flatten())
    });
    let anchor = rect.map(|r| {
        // Tray rects are converted using the monitor scale below. Tauri tray
        // events currently provide physical coordinates on macOS and Windows.
        let p = r.position.to_physical::<f64>(1.0);
        let s = r.size.to_physical::<f64>(1.0);
        (p.x, p.y, s.width, s.height)
    });
    place(app, &window, anchor)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    app.emit_to("overview", "companion-visibility", true)
        .map_err(|e| e.to_string())
}
/// Fob: give the mini window its fixed size and put it where the user
/// dragged it (clamped into that monitor's work area) or, by default or when
/// that monitor is gone, next to the tray icon (above it for a bottom
/// taskbar). Only `show` calls this.
fn place(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    anchor: Option<(f64, f64, f64, f64)>,
) -> Result<(), String> {
    let (anchor, saved) = {
        let state = app.state::<Mutex<CompanionState>>();
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if anchor.is_some() {
            state.anchor = anchor;
        }
        (state.anchor, state.user_rect)
    };
    let monitors: Vec<(PxRect, f64)> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            (PxRect::new(p.x, p.y, s.width, s.height), m.scale_factor())
        })
        .collect();
    let restored = saved.and_then(|rect| {
        let (bounds, scale) = monitor_of(&rect, &monitors)?;
        let size = mini_size(scale);
        let screen = screen_at(center(&overlap(&rect, &bounds)?), bounds);
        Some((screen, scale, size, restore(rect, &screen.work, size)))
    });
    let (source, (screen, scale, size, placed)) = match (saved, restored) {
        (Some(_), Some(found)) => ("dragged position".to_owned(), found),
        (Some(rect), None) => (
            format!("dragged position {rect:?} is on no monitor {monitors:?}: tray"),
            at_tray(app, window, anchor)?,
        ),
        (None, _) => ("tray".to_owned(), at_tray(app, window, anchor)?),
    };
    if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
        state.placed = Some((placed.rect(), Instant::now()));
    }
    #[cfg(target_os = "windows")]
    {
        let result = set_window_rect(window, placed.rect());
        if let (Ok((_, visible)), Ok(mut state)) =
            (&result, app.state::<Mutex<CompanionState>>().lock())
        {
            state.placed = Some((*visible, Instant::now()));
        }
        record_placement(
            app,
            &source,
            &screen,
            anchor,
            scale,
            size,
            &placed,
            result.as_ref().ok(),
        );
        result.map(|_| ())
    }
    #[cfg(not(target_os = "windows"))]
    {
        record_placement(app, &source, &screen, anchor, scale, size, &placed, None);
        // Position first, then size: the window is already on the target
        // monitor when it gets its size.
        window.set_position(placed.pos).map_err(|e| e.to_string())?;
        window
            .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                placed.size.0,
                placed.size.1,
            )))
            .map_err(|e| e.to_string())
    }
}
/// The tray-anchored placement: screen, scale, panel size and rect.
fn at_tray(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    anchor: Option<(f64, f64, f64, f64)>,
) -> Result<(Screen, f64, tauri::PhysicalSize<u32>, Placed), String> {
    let monitor = anchor
        .and_then(|(x, y, _, _)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| {
            app.get_webview_window("main")
                .and_then(|w| w.current_monitor().ok().flatten())
        })
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or("No display available")?;
    let scale = monitor.scale_factor();
    let size = mini_size(scale);
    let bounds = PxRect::new(
        monitor.position().x,
        monitor.position().y,
        monitor.size().width,
        monitor.size().height,
    );
    let point = anchor.map_or(
        (
            (bounds.left + bounds.right) as f64 / 2.0,
            (bounds.top + bounds.bottom) as f64 / 2.0,
        ),
        |(x, y, w, h)| (x + w / 2.0, y + h / 2.0),
    );
    let screen = screen_at(point, bounds);
    let placed = placement(&screen, (size.width, size.height), anchor, scale);
    Ok((screen, scale, size, placed))
}
fn center(r: &PxRect) -> (f64, f64) {
    (
        (r.left as f64 + r.right as f64) / 2.0,
        (r.top as f64 + r.bottom as f64) / 2.0,
    )
}
/// The part of `a` inside `b`, if they overlap.
fn overlap(a: &PxRect, b: &PxRect) -> Option<PxRect> {
    let r = PxRect {
        left: a.left.max(b.left),
        top: a.top.max(b.top),
        right: a.right.min(b.right),
        bottom: a.bottom.min(b.bottom),
    };
    (r.right > r.left && r.bottom > r.top).then_some(r)
}
/// The monitor (rect, scale) holding most of `rect`, if one still overlaps
/// it. A window dragged half off the bottom has its centre off-screen but
/// still belongs to that monitor.
fn monitor_of(rect: &PxRect, monitors: &[(PxRect, f64)]) -> Option<(PxRect, f64)> {
    let area = |r: PxRect| (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
    monitors
        .iter()
        .copied()
        .filter_map(|(m, s)| Some((area(overlap(rect, &m)?), (m, s))))
        .max_by_key(|(a, _)| *a)
        .map(|(_, found)| found)
}
/// Fob: the dragged position at the panel size of the monitor it's on, kept
/// at its top-left but moved (and if need be shrunk) fully into `work`.
fn restore(saved: PxRect, work: &PxRect, panel: tauri::PhysicalSize<u32>) -> Placed {
    let (aw, ah) = (work.right - work.left, work.bottom - work.top);
    let (pw, ph) = (
        (panel.width as i32).min(aw).max(1),
        (panel.height as i32).min(ah).max(1),
    );
    Placed {
        pos: PhysicalPosition::new(
            saved.left.clamp(work.left, work.right - pw),
            saved.top.clamp(work.top, work.bottom - ph),
        ),
        size: (pw as u32, ph as u32),
        fits: panel.width as i32 <= aw && panel.height as i32 <= ah,
    }
}
/// Fob: the last placement's numbers, overwritten on every show, in the app
/// log folder (mini-placement.txt) so a misplaced window can be explained.
#[allow(clippy::too_many_arguments)]
fn record_placement(
    app: &AppHandle,
    source: &str,
    screen: &Screen,
    anchor: Option<(f64, f64, f64, f64)>,
    scale: f64,
    size: tauri::PhysicalSize<u32>,
    placed: &Placed,
    actual: Option<&(PxRect, PxRect)>,
) {
    let mut text = format!(
        "source {source}\nscale {scale}\nmonitor {:?}\nwork {:?}\ntaskbar {:?}\ntray icon {:?}\n\
         panel wanted {}x{}\nplanned {:?}\nfits work area: {}\n",
        screen.monitor,
        screen.work,
        screen.taskbar,
        anchor,
        size.width,
        size.height,
        placed.rect(),
        placed.fits,
    );
    if let Some((window, visible)) = actual {
        text += &format!(
            "window rect {window:?}\nvisible rect {visible:?}\ninside work area: {}\n",
            contains(&screen.work, visible)
        );
    }
    write_placement(app, &text);
}
/// mini-placement.txt: the last placement or drag; mini-placement.log: one
/// line per event.
fn write_placement(app: &AppHandle, text: &str) {
    let Ok(dir) = app.path().app_log_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("mini-placement.txt"), text);
    // And one line per show in mini-placement.log (restarted past 256 KB).
    let log = dir.join("mini-placement.log");
    if std::fs::metadata(&log).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&log);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let line = format!("{stamp} {}\n", text.trim_end().replace('\n', " | "));
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
    {
        let _ = file.write_all(line.as_bytes());
    }
}
/// Fob: mini-position.json in the settings folder: the dragged visible rect.
fn position_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("mini-position.json"))
}
fn rect_from_json(value: &serde_json::Value) -> Option<PxRect> {
    let n = |k: &str| value[k].as_i64().and_then(|v| i32::try_from(v).ok());
    let rect = PxRect {
        left: n("left")?,
        top: n("top")?,
        right: n("right")?,
        bottom: n("bottom")?,
    };
    (rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
}
fn load_position(app: &AppHandle) -> Option<PxRect> {
    let bytes = std::fs::read(position_file(app)?).ok()?;
    rect_from_json(&serde_json::from_slice(&bytes).ok()?)
}
fn save_position(app: &AppHandle, rect: Option<PxRect>) {
    let Some(path) = position_file(app) else {
        return;
    };
    match rect {
        Some(r) => {
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
            let json = serde_json::json!({
                "left": r.left, "top": r.top, "right": r.right, "bottom": r.bottom
            });
            let _ = std::fs::write(&path, json.to_string());
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}
/// The window's visible rect now (Windows: DWM frame bounds).
fn visible_rect(window: &tauri::WebviewWindow) -> Option<PxRect> {
    #[cfg(target_os = "windows")]
    {
        window_rects(window).ok().map(|(_, visible)| visible)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let (p, s) = (window.outer_position().ok()?, window.outer_size().ok()?);
        Some(PxRect::new(p.x, p.y, s.width, s.height))
    }
}
/// A move that `place` did not make, within this long after it, is still
/// taken as its own (Windows applies the rect in steps).
const OWN_MOVE: Duration = Duration::from_millis(400);
/// Saved once the window has been still this long (a drag is many moves).
const SETTLE: Duration = Duration::from_millis(500);
/// Is this move the user's drag? Only a pinned, visible window can be
/// dragged; `place`'s own rect (and its moves just after) are not drags.
fn is_drag(pinned: bool, placed: Option<(PxRect, Instant)>, now: PxRect) -> bool {
    pinned
        && match placed {
            Some((rect, at)) => rect != now && at.elapsed() >= OWN_MOVE,
            None => true,
        }
}
/// Fob: the overview window moved. A drag of the pinned window is kept in
/// memory and saved when the window settles, then wins over the tray spot.
fn moved(app: &AppHandle) {
    let Some(window) = app.get_webview_window("overview") else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let Some(now) = visible_rect(&window) else {
        return;
    };
    let service = app.state::<Mutex<CompanionState>>();
    let Ok(mut state) = service.lock() else {
        return;
    };
    if !is_drag(state.pinned, state.placed, now) {
        return;
    }
    state.moved = Some((now, Instant::now()));
    if std::mem::replace(&mut state.saving, true) {
        return; // a saver is already waiting
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(200));
        let service = app.state::<Mutex<CompanionState>>();
        let Ok(mut state) = service.lock() else {
            return;
        };
        let Some((rect, at)) = state.moved else {
            state.saving = false;
            return;
        };
        if at.elapsed() < SETTLE {
            continue;
        }
        state.moved = None;
        state.saving = false;
        state.user_rect = Some(rect);
        state.placed = Some((rect, Instant::now()));
        drop(state);
        save_position(&app, Some(rect));
        write_placement(
            &app,
            &format!("source dragged\nsaved visible rect {rect:?}\n"),
        );
        return;
    });
}
/// Fob: double-click on the mini title bar: forget the dragged position and
/// go back to the tray.
#[tauri::command]
pub async fn reset_mini_position(app: AppHandle) -> Result<(), String> {
    if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
        state.user_rect = None;
        state.moved = None;
    }
    save_position(&app, None);
    write_placement(&app, "source reset to the tray position\n");
    let visible = app
        .get_webview_window("overview")
        .is_some_and(|w| w.is_visible().unwrap_or(false));
    if visible {
        show(&app, None)?;
    }
    Ok(())
}
/// Whether the mini window has a dragged position (Settings shows reset).
#[tauri::command]
pub async fn get_mini_position_saved(
    state: tauri::State<'_, Mutex<CompanionState>>,
) -> Result<bool, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.user_rect.is_some())
}
/// Fob: an unpinned mini window closes like a flyout when another window gets
/// focus; a pinned one stays open (and on top) until the tray icon, Esc or
/// "open full window" hides it.
fn hides_on_focus_loss(pinned: bool) -> bool {
    !pinned
}
/// Fob: the mini window's pin button. Saved as behavior.miniPinned.
#[tauri::command]
pub async fn get_companion_pinned(
    state: tauri::State<'_, Mutex<CompanionState>>,
) -> Result<bool, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.pinned)
}
#[tauri::command]
pub async fn set_companion_pinned(app: AppHandle, pinned: bool) -> Result<bool, String> {
    let before = {
        let state = app.state::<Mutex<CompanionState>>();
        let mut state = state.lock().map_err(|e| e.to_string())?;
        std::mem::replace(&mut state.pinned, pinned)
    };
    let saver = app.clone();
    let saved = tauri::async_runtime::spawn_blocking(move || {
        let patch = serde_json::json!({ "behavior": { "miniPinned": pinned } });
        crate::preferences::apply_patch(&saver, &patch).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    if let Err(e) = saved {
        if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
            state.pinned = before; // not saved: keep the old behaviour
        }
        return Err(e);
    }
    Ok(pinned)
}
pub fn open_main(app: &AppHandle) -> Result<(), String> {
    hide(app);
    let window = app
        .get_webview_window("main")
        .ok_or("Main window unavailable")?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}
pub fn window_event(window: &tauri::Window, event: &WindowEvent) {
    let app = window.app_handle();
    if let WindowEvent::ScaleFactorChanged { .. } = event {
        crate::window_icon::apply(window);
    }
    if window.label() == "overview"
        && matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        )
    {
        #[cfg(target_os = "windows")]
        if let Ok(h) = window.hwnd() {
            crate::ui::apply_mini_shape(h.0 as isize, window.scale_factor().unwrap_or(1.0));
        }
    }
    match (window.label(), event) {
        ("overview", WindowEvent::Moved(_)) => moved(app),
        ("overview", WindowEvent::Focused(false)) => {
            let pinned = app
                .state::<Mutex<CompanionState>>()
                .lock()
                .is_ok_and(|s| s.pinned);
            if hides_on_focus_loss(pinned) {
                hide(app);
            }
        }
        ("overview", WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            hide(app);
        }
        ("settings" | "about", WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            let _ = window.hide();
        }
        ("main", WindowEvent::CloseRequested { api, .. }) => {
            let keep = app
                .state::<Mutex<CompanionState>>()
                .lock()
                .map(|s| s.keep_running)
                .unwrap_or(false);
            api.prevent_close();
            if keep {
                let _ = window.hide();
            } else {
                app.exit(0);
            }
        }
        _ => {}
    }
}

#[tauri::command]
pub async fn configure_companion(
    app: AppHandle,
    enabled: bool,
    keep_running: bool,
    start_in_tray: Option<bool>,
    language: Option<String>,
) -> Result<(), String> {
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let result = configure(
            &handle,
            enabled,
            keep_running,
            start_in_tray,
            language.as_deref(),
        );
        let _ = sender.try_send(result);
    })
    .map_err(|e| e.to_string())?;
    receiver
        .recv()
        .await
        .ok_or("Companion configuration was interrupted")?
}
#[tauri::command]
pub async fn show_companion(app: AppHandle) -> Result<(), String> {
    show(&app, None)
}
#[tauri::command]
pub async fn hide_companion(app: AppHandle) {
    hide(&app);
}
#[derive(Deserialize, Serialize, Clone)]
pub struct ProcessTarget {
    pid: u32,
    start_time: u64,
}
#[tauri::command]
pub async fn open_main_window(
    app: AppHandle,
    process: Option<ProcessTarget>,
    page: Option<String>,
) -> Result<(), String> {
    // A page of the main window to show (the disk space map).
    if let Some(page) = page.filter(|p| p == "space") {
        open_main(&app)?;
        return app
            .emit_to("main", "open-page", page)
            .map_err(|e| e.to_string());
    }
    if let Some(target) = process {
        let state = app.state::<crate::state::AppState>();
        let frame = state
            .hub
            .lock()
            .map_err(|e| e.to_string())?
            .frame
            .clone()
            .ok_or("No process readings yet")?;
        if !frame
            .processes
            .iter()
            .any(|p| p.pid == target.pid && p.start_time == target.start_time)
        {
            return Err("This process is no longer available".into());
        }
        open_main(&app)?;
        app.emit_to("main", "open-process", target)
            .map_err(|e| e.to_string())
    } else {
        open_main(&app)
    }
}
#[tauri::command]
pub async fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    #[test]
    fn tray_frame_follows_display_scale() {
        use super::tray_frame_size;
        assert_eq!(tray_frame_size(1.0), 16);
        assert_eq!(tray_frame_size(1.25), 24);
        assert_eq!(tray_frame_size(1.5), 24);
        assert_eq!(tray_frame_size(1.75), 32);
        assert_eq!(tray_frame_size(2.0), 32);
        assert_eq!(tray_frame_size(3.0), 32);
        assert_eq!(tray_frame_size(f64::NAN), 16);
    }

    #[test]
    fn tray_frames_decode_at_their_size() {
        for (size, bytes) in super::TRAY_FRAMES {
            let image = tauri::image::Image::from_bytes(bytes).unwrap();
            assert_eq!((image.width(), image.height()), (size, size));
        }
    }

    use super::*;
    #[test]
    fn focus_loss_hides_only_an_unpinned_mini_window() {
        assert!(hides_on_focus_loss(false));
        assert!(!hides_on_focus_loss(true));
        // The focus-loss handler is the only automatic hide and goes through it.
        let src = include_str!("companion.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        let handler = code
            .split(r#"("overview", WindowEvent::Focused(false))"#)
            .nth(1)
            .unwrap();
        let body = &handler[..handler.find("WindowEvent::CloseRequested").unwrap()];
        assert!(body.contains("hides_on_focus_loss(pinned)"));
    }
    #[test]
    fn mini_height_matches_the_frontend_layout() {
        let js = include_str!("../../src/lib/utils/mini-layout.js");
        let line = js
            .lines()
            .find(|l| l.starts_with("export const MINI_HEIGHT = "))
            .expect("MINI_HEIGHT in mini-layout.js");
        let value: f64 = line
            .trim_start_matches("export const MINI_HEIGHT = ")
            .trim_end_matches(';')
            .parse()
            .unwrap();
        assert_eq!(value, MINI_HEIGHT);
        assert!(js.contains("width: 360,") && MINI_WIDTH == 360.0);
    }
    #[test]
    fn nothing_but_show_sizes_or_moves_the_mini_window() {
        // A blank-area click used to re-measure the page and call
        // fit_companion -> set_size + set_position, moving the whole window.
        let src = include_str!("companion.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        let callers = |needle: &str| code.matches(needle).count();
        // `place`: the fn and its one call in `show`. Windows: one exact
        // SetWindowPos path; elsewhere set_position + set_size.
        assert_eq!(callers(concat!(" place", "(")), 2);
        assert_eq!(callers(concat!("set_window", "_rect(")), 2);
        assert_eq!(callers(concat!("SetWindow", "Pos(")), 1);
        assert_eq!(callers(concat!(".set_", "size(")), 1);
        assert_eq!(callers(concat!(".set_", "position(")), 1);
        assert!(!code.contains(concat!("fit_", "companion")));
    }
    /// A 1920x1080 monitor at `scale` with a 48 px (logical) taskbar on `edge`.
    fn hd(scale: f64, edge: Edge, origin: (i32, i32)) -> (Screen, (f64, f64, f64, f64)) {
        let (w, h) = ((1920.0 * scale) as u32, (1080.0 * scale) as u32);
        let m = PxRect::new(origin.0, origin.1, w, h);
        let t = (48.0 * scale).round() as i32;
        let (bar, icon) = match edge {
            Edge::Bottom => (
                PxRect {
                    top: m.bottom - t,
                    ..m
                },
                (
                    m.right as f64 - 300.0 * scale,
                    (m.bottom - t) as f64,
                    24.0 * scale,
                    t as f64,
                ),
            ),
            Edge::Top => (
                PxRect {
                    bottom: m.top + t,
                    ..m
                },
                (
                    m.right as f64 - 300.0 * scale,
                    m.top as f64,
                    24.0 * scale,
                    t as f64,
                ),
            ),
            Edge::Left => (
                PxRect {
                    right: m.left + t,
                    ..m
                },
                (
                    m.left as f64,
                    m.bottom as f64 - 200.0 * scale,
                    t as f64,
                    24.0 * scale,
                ),
            ),
            Edge::Right => (
                PxRect {
                    left: m.right - t,
                    ..m
                },
                (
                    (m.right - t) as f64,
                    m.bottom as f64 - 200.0 * scale,
                    t as f64,
                    24.0 * scale,
                ),
            ),
        };
        let work = match edge {
            Edge::Bottom => PxRect {
                bottom: bar.top,
                ..m
            },
            Edge::Top => PxRect {
                top: bar.bottom,
                ..m
            },
            Edge::Left => PxRect {
                left: bar.right,
                ..m
            },
            Edge::Right => PxRect {
                right: bar.left,
                ..m
            },
        };
        (
            Screen {
                monitor: m,
                work,
                taskbar: Some((edge, bar, false)),
            },
            icon,
        )
    }
    fn inside(p: &Placed, size: tauri::PhysicalSize<u32>, area: &PxRect, gap: i32) {
        assert!(p.fits);
        assert_eq!(p.size, (size.width, size.height));
        assert!(
            p.pos.x >= area.left + gap && p.pos.y >= area.top + gap,
            "{p:?} {area:?}"
        );
        assert!(
            p.pos.x + size.width as i32 <= area.right - gap,
            "{p:?} {area:?}"
        );
        assert!(
            p.pos.y + size.height as i32 <= area.bottom - gap,
            "{p:?} {area:?}"
        );
    }
    #[test]
    fn bottom_taskbar_100_125_150_percent_stays_above_the_taskbar() {
        for scale in [1.0, 1.25, 1.5, 1.75] {
            let (screen, icon) = hd(scale, Edge::Bottom, (0, 0));
            let size = mini_size(scale);
            let gap = (12.0 * scale).round() as i32;
            let p = placement(&screen, (size.width, size.height), Some(icon), scale);
            inside(&p, size, &screen.work, gap);
            // Bottom edge exactly one gap above the taskbar, centred on the icon.
            assert_eq!(p.pos.y + size.height as i32, screen.work.bottom - gap);
            let cx = icon.0 + icon.2 / 2.0;
            assert_eq!(p.pos.x, (cx - size.width as f64 / 2.0).round() as i32);
        }
        // The v0.5.3 numbers at 150 %: 2880x1620, taskbar 72 px, panel 540x978.
        let (screen, icon) = hd(1.5, Edge::Bottom, (0, 0));
        let p = placement(&screen, (540, 978), Some(icon), 1.5);
        assert_eq!(p.pos.y, 1620 - 72 - 18 - 978);
    }
    #[test]
    fn the_work_area_wins_over_the_monitor_and_a_bad_icon_rect() {
        // v0.5.3 decided above/below from the icon rect and clamped to the
        // monitor: an icon rect near the top put the panel at the monitor
        // bottom, over the taskbar. The work area decides now.
        let (screen, _) = hd(1.0, Edge::Bottom, (0, 0));
        let size = mini_size(1.0);
        for icon in [
            (1600.0, 0.0, 24.0, 24.0),
            (1600.0, 1079.0, 24.0, 1.0),
            (5000.0, 5000.0, 24.0, 24.0),
        ] {
            let p = placement(&screen, (size.width, size.height), Some(icon), 1.0);
            inside(&p, size, &screen.work, 12);
            assert_eq!(p.pos.y + size.height as i32, 1080 - 48 - 12);
        }
        let p = placement(&screen, (size.width, size.height), None, 1.0);
        assert_eq!(
            p.pos,
            PhysicalPosition::new(1920 - 12 - 360, 1080 - 48 - 12 - 652)
        );
    }
    #[test]
    fn taskbar_on_top_left_and_right() {
        for scale in [1.0, 1.25, 1.5, 1.75] {
            let size = mini_size(scale);
            let gap = (12.0 * scale).round() as i32;
            for edge in [Edge::Top, Edge::Left, Edge::Right] {
                let (screen, icon) = hd(scale, edge, (0, 0));
                let p = placement(&screen, (size.width, size.height), Some(icon), scale);
                inside(&p, size, &screen.work, gap);
                match edge {
                    Edge::Top => assert_eq!(p.pos.y, screen.work.top + gap),
                    Edge::Left => assert_eq!(p.pos.x, screen.work.left + gap),
                    Edge::Right => {
                        assert_eq!(p.pos.x + size.width as i32, screen.work.right - gap)
                    }
                    Edge::Bottom => unreachable!(),
                }
            }
        }
    }
    #[test]
    fn auto_hidden_taskbar_keeps_its_strip_free() {
        let (mut screen, icon) = hd(1.25, Edge::Bottom, (0, 0));
        screen.work = screen.monitor; // auto-hide: Windows reserves nothing
        let bar = screen.taskbar.unwrap().1;
        screen.taskbar = Some((Edge::Bottom, bar, true));
        let size = mini_size(1.25);
        let p = placement(&screen, (size.width, size.height), Some(icon), 1.25);
        assert_eq!(p.pos.y + size.height as i32, bar.top - 15);
    }
    #[test]
    fn secondary_monitor_with_negative_origin() {
        let (mut screen, icon) = hd(1.0, Edge::Bottom, (-1920, -200));
        // The primary taskbar (ABM_GETTASKBARPOS) is on another monitor.
        screen.taskbar = Some((Edge::Bottom, PxRect::new(0, 1032, 1920, 48), false));
        let size = mini_size(1.0);
        let p = placement(&screen, (size.width, size.height), Some(icon), 1.0);
        inside(&p, size, &screen.work, 12);
        assert_eq!(p.pos.y + 652, 880 - 48 - 12);
    }
    #[test]
    fn mac_menu_bar_at_the_top_without_a_work_area() {
        let m = PxRect::new(0, 0, 2880, 1800);
        let screen = Screen {
            monitor: m,
            work: m,
            taskbar: None,
        };
        let p = placement(&screen, (720, 1304), Some((2800.0, 0.0, 40.0, 48.0)), 2.0);
        assert_eq!(p.pos.y, 24);
        assert!(p.fits);
    }
    #[test]
    fn too_tall_for_the_work_area_shrinks_and_still_clears_the_taskbar() {
        // 1920x1080 at 175 %: 1141 px panel, 1080 - 84 px taskbar = 996 px work area.
        let m = PxRect::new(0, 0, 1920, 1080);
        let screen = Screen {
            monitor: m,
            work: PxRect { bottom: 996, ..m },
            taskbar: Some((Edge::Bottom, PxRect { top: 996, ..m }, false)),
        };
        let size = mini_size(1.75);
        assert_eq!(size.height, 1141);
        let p = placement(
            &screen,
            (size.width, size.height),
            Some((1500.0, 996.0, 42.0, 84.0)),
            1.75,
        );
        assert!(!p.fits);
        let gap = 21; // 12 px at 175 %
        assert_eq!(p.size, (size.width, 996 - 2 * gap as u32));
        assert_eq!((p.pos.y, p.rect().bottom), (gap, 996 - gap));
        assert!(contains(&screen.work, &p.rect()));
    }
    #[test]
    fn negative_coordinates_at_every_scale_and_edge() {
        // A monitor left of and above the primary one (negative origin).
        for scale in [1.0, 1.25, 1.5, 1.75] {
            for edge in [Edge::Bottom, Edge::Top, Edge::Left, Edge::Right] {
                let (screen, icon) = hd(scale, edge, (-2400, -1300));
                let size = mini_size(scale);
                let gap = (12.0 * scale).round() as i32;
                let p = placement(&screen, (size.width, size.height), Some(icon), scale);
                inside(&p, size, &screen.work, gap);
            }
        }
    }
    #[test]
    fn the_visible_rect_lands_on_the_plan_despite_invisible_borders() {
        let planned = PxRect::new(1300, 300, 540, 978);
        // A window whose rect is 7 px wider on the left/right/bottom than
        // what DWM shows (classic invisible resize borders), currently
        // somewhere else and the wrong size.
        let window = PxRect::new(100, 100, 600, 1050);
        let visible = PxRect {
            left: window.left + 7,
            top: window.top,
            right: window.right - 7,
            bottom: window.bottom - 7,
        };
        let target = window_rect_for(planned, window, visible);
        assert_eq!(
            target,
            PxRect {
                left: 1293,
                top: 300,
                right: 1847,
                bottom: 1285
            }
        );
        // Without borders the window rect is the plan itself.
        assert_eq!(window_rect_for(planned, window, window), planned);
    }
    #[test]
    fn the_v0_6_0_window_was_taller_than_planned() {
        // What v0.6.0 did at 150 %: planned 540x978 above a 72 px taskbar,
        // but the window kept its creation size (title bar + frame added by
        // AdjustWindowRectEx, ~45 px taller), so its bottom crossed the
        // taskbar. Planning with the window's real height shows the overlap;
        // the fix sets the exact rect and checks the visible one.
        let (screen, icon) = hd(1.5, Edge::Bottom, (0, 0));
        let p = placement(&screen, (540, 978), Some(icon), 1.5);
        let actual = PxRect::new(p.pos.x, p.pos.y, 540 + 12, 978 + 45);
        assert!(!contains(&screen.work, &actual));
        assert!(contains(&screen.work, &p.rect()));
    }
    #[test]
    fn the_first_launch_shows_the_main_window_even_in_tray_only_mode() {
        assert!(shows_main_at_launch(true, true));
        assert!(!shows_main_at_launch(true, false));
        assert!(shows_main_at_launch(false, false));
        assert!(shows_main_at_launch(false, true));
    }
    #[test]
    fn only_a_pinned_window_moved_by_someone_else_is_a_drag() {
        let placed = PxRect::new(100, 100, 360, 652);
        let moved = PxRect::new(300, 50, 360, 652);
        let long_ago = Instant::now() - Duration::from_secs(2);
        assert!(is_drag(true, Some((placed, long_ago)), moved));
        assert!(!is_drag(false, Some((placed, long_ago)), moved), "unpinned");
        assert!(!is_drag(true, Some((placed, long_ago)), placed), "our rect");
        assert!(
            !is_drag(true, Some((placed, Instant::now())), moved),
            "just placed: our own intermediate move"
        );
        assert!(is_drag(true, None, moved));
    }
    #[test]
    fn a_dragged_position_comes_back_inside_its_monitor() {
        let left = (PxRect::new(-1920, 0, 1920, 1080), 1.0);
        let main = (PxRect::new(0, 0, 2880, 1620), 1.5);
        let monitors = [left, main];
        let size = mini_size(1.5);
        // On the main monitor, fully inside: kept as is.
        let saved = PxRect::new(1000, 200, size.width, size.height);
        assert_eq!(monitor_of(&saved, &monitors), Some(main));
        let work = PxRect::new(0, 0, 2880, 1548); // 72 px taskbar
        let p = restore(saved, &work, size);
        assert_eq!(p.rect(), saved);
        assert!(p.fits);
        // Hanging over the taskbar and the right edge: pulled inside.
        let over = PxRect::new(2700, 1200, size.width, size.height);
        let p = restore(over, &work, size);
        assert!(contains(&work, &p.rect()), "{p:?}");
        assert_eq!(p.size, (size.width, size.height));
        // Saved on the left monitor at 100 %, which now is the panel size
        // there (the caller passes mini_size(1.0)).
        let on_left = PxRect::new(-800, 300, 360, 652);
        assert_eq!(monitor_of(&on_left, &monitors), Some(left));
        // Dragged half off the bottom: the centre is off-screen, the monitor
        // is still the one it hangs from, and the restore pulls it inside.
        let hanging = PxRect::new(1500, 1300, size.width, size.height);
        assert_eq!(monitor_of(&hanging, &monitors), Some(main));
        assert!(contains(&work, &restore(hanging, &work, size).rect()));
        // That monitor unplugged: no monitor, so the tray position is used.
        assert_eq!(monitor_of(&on_left, &[main]), None);
        // A work area smaller than the panel: shrunk, still inside.
        let tiny = PxRect::new(0, 0, 800, 600);
        let p = restore(saved, &tiny, size);
        assert!(!p.fits && contains(&tiny, &p.rect()));
    }
    #[test]
    fn the_saved_position_file_rejects_junk() {
        let ok = serde_json::json!({"left":-10,"top":5,"right":350,"bottom":657});
        assert_eq!(rect_from_json(&ok), Some(PxRect::new(-10, 5, 360, 652)));
        for bad in [
            serde_json::json!({"left":0,"top":0,"right":0,"bottom":10}),
            serde_json::json!({"left":"0","top":0,"right":5,"bottom":10}),
            serde_json::json!({"left":0,"top":0,"right":5}),
            serde_json::json!(null),
        ] {
            assert_eq!(rect_from_json(&bad), None, "{bad}");
        }
    }
}
