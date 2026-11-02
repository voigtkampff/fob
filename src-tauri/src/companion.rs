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
    /// winperf: content height reported by the mini window (logical px).
    pub height: Option<f64>,
    /// winperf: last tray icon rect (physical px), so resizes stay anchored.
    pub anchor: Option<(f64, f64, f64, f64)>,
    /// winperf: tray menu "Start in tray only" item, kept in sync with settings.
    pub start_item: Option<CheckMenuItem<tauri::Wry>>,
    /// winperf: tray items whose text follows the UI language.
    pub items: Option<TrayItems>,
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

/// winperf mini window: fixed width, height follows its content.
const MINI_WIDTH: f64 = 360.0;
const MINI_MIN_HEIGHT: f64 = 240.0;
const MINI_MAX_HEIGHT: f64 = 720.0;
const MINI_DEFAULT_HEIGHT: f64 = 640.0;

pub fn mini_height(content: f64) -> f64 {
    if content.is_finite() {
        content.clamp(MINI_MIN_HEIGHT, MINI_MAX_HEIGHT)
    } else {
        MINI_DEFAULT_HEIGHT
    }
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
/// appears; the first launch also pops the mini window once so it's clear
/// where winperf went. Otherwise the main window opens as before.
pub fn launch(app: &AppHandle) -> Result<(), String> {
    let startup = crate::preferences::startup(app);
    // No tray (or it can't be created): the main window must appear.
    if startup.tray
        && startup.start_in_tray
        && configure(app, true, true, Some(true), startup.language.as_deref()).is_ok()
    {
        // Debug builds: WINPERF_SHOW_MINI=1 pops it every time (screenshots).
        let forced = cfg!(debug_assertions) && std::env::var_os("WINPERF_SHOW_MINI").is_some();
        if startup.first_launch || forced {
            show(app, None)?;
        }
        Ok(())
    } else {
        let window = app
            .get_webview_window("main")
            .ok_or("Main window unavailable")?;
        window.show().map_err(|e| e.to_string())
    }
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
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        };
        if let Ok(hwnd) = window.hwnd() {
            let preference = DWMWCP_ROUND;
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

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overview") {
        let _ = window.hide();
    }
    if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
        state.hidden_at = Some(Instant::now());
    }
    let _ = app.emit_to("overview", "companion-visibility", false);
}

// All geometry is physical pixels. Negative monitor origins and scale factors
// must not be mixed with logical popup dimensions.
fn placement(
    bounds: (i32, i32, u32, u32),
    panel: (u32, u32),
    anchor: Option<(f64, f64, f64, f64)>,
    scale: f64,
) -> PhysicalPosition<i32> {
    let (left, top, width, height) = bounds;
    let (pw, ph) = panel;
    let margin = (8.0 * scale).round() as i32;
    let right = left.saturating_add(width as i32);
    let bottom = top.saturating_add(height as i32);
    let (x, y) = if let Some((ax, ay, aw, ah)) = anchor {
        let x = (ax + aw / 2.0 - pw as f64 / 2.0).round() as i32;
        // A small gap to the taskbar: the native shadow needs room.
        let gap = margin as f64;
        let y = if ay + ah + gap + ph as f64 <= bottom as f64 {
            (ay + ah + gap).round() as i32
        } else {
            (ay - gap - ph as f64).round() as i32
        };
        (x, y)
    } else {
        (right - pw as i32 - margin, top + (32.0 * scale) as i32)
    };
    PhysicalPosition::new(
        x.clamp(left, (right - pw as i32).max(left)),
        y.clamp(top, (bottom - ph as i32).max(top)),
    )
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
/// winperf: size the mini window to its content height and put it next to the
/// tray icon (above it for a bottom taskbar), clamped to the monitor.
fn place(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    anchor: Option<(f64, f64, f64, f64)>,
) -> Result<(), String> {
    let (anchor, height) = {
        let state = app.state::<Mutex<CompanionState>>();
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if anchor.is_some() {
            state.anchor = anchor;
        }
        (state.anchor, state.height.unwrap_or(MINI_DEFAULT_HEIGHT))
    };
    let monitor = anchor
        .and_then(|(x, y, _, _)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| {
            app.get_webview_window("main")
                .and_then(|w| w.current_monitor().ok().flatten())
        })
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or("No display available")?;
    let scale = monitor.scale_factor();
    // Round up: a pixel lost to rounding at 125 %/150 % shows a scrollbar.
    let size = tauri::PhysicalSize::new(
        (MINI_WIDTH * scale).ceil() as u32,
        (height * scale).ceil() as u32,
    );
    window
        .set_size(tauri::Size::Physical(size))
        .map_err(|e| e.to_string())?;
    window
        .set_position(placement(
            (
                monitor.position().x,
                monitor.position().y,
                monitor.size().width,
                monitor.size().height,
            ),
            (size.width, size.height),
            anchor,
            scale,
        ))
        .map_err(|e| e.to_string())
}
/// winperf: the mini window reports its content height; resize and re-anchor.
#[tauri::command]
pub async fn fit_companion(app: AppHandle, height: f64) -> Result<(), String> {
    let height = mini_height(height);
    {
        let state = app.state::<Mutex<CompanionState>>();
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if state.height.is_some_and(|h| (h - height).abs() < 1.0) {
            return Ok(());
        }
        state.height = Some(height);
    }
    let window = app
        .get_webview_window("overview")
        .ok_or("Overview window unavailable")?;
    place(&app, &window, None)
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
    match (window.label(), event) {
        ("overview", WindowEvent::Focused(false)) => hide(app),
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
) -> Result<(), String> {
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
    fn panel_stays_on_scaled_and_negative_displays() {
        assert_eq!(
            placement(
                (0, 0, 2880, 1800),
                (800, 1200),
                Some((2800.0, 0.0, 40.0, 48.0)),
                2.0
            ),
            PhysicalPosition::new(2080, 64) // below a top menu bar, 16 px gap at 200%
        );
        assert_eq!(
            placement(
                (-1920, 0, 1920, 1080),
                (400, 600),
                Some((-1900.0, 1040.0, 24.0, 40.0)),
                1.0
            ),
            PhysicalPosition::new(-1920, 432) // 8 px above the icon
        );
        assert_eq!(
            placement((0, 0, 300, 400), (400, 600), None, 1.0),
            PhysicalPosition::new(0, 0)
        );
    }
    #[test]
    fn mini_window_sits_above_a_bottom_right_tray_icon() {
        // 1920x1080 at 150%: 360x600 logical = 540x900 physical; taskbar icon at the bottom right.
        let pos = placement(
            (0, 0, 2880, 1620),
            (540, 900),
            Some((2600.0, 1560.0, 36.0, 60.0)),
            1.5,
        );
        assert_eq!(pos, PhysicalPosition::new(2340, 648)); // 12 px gap at 150%
    }
    #[test]
    fn mini_height_is_clamped() {
        assert_eq!(mini_height(100.0), 240.0);
        assert_eq!(mini_height(500.0), 500.0);
        assert_eq!(mini_height(5000.0), 720.0);
        assert_eq!(mini_height(f64::NAN), 640.0);
    }
}
