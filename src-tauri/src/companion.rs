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
/// appears; the first launch also pops the mini window once so it's clear
/// where winperf went. Otherwise the main window opens as before.
pub fn launch(app: &AppHandle) -> Result<(), String> {
    let startup = crate::preferences::startup(app);
    if let Ok(mut state) = app.state::<Mutex<CompanionState>>().lock() {
        state.pinned = startup.mini_pinned;
    }
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
    /// false: the panel is larger than the work area (minus gaps).
    fits: bool,
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
    // Too big for the work area: keep the top-left visible rather than
    // pushing it off screen (the caller reports it).
    let clamp =
        |v: i32, lo: i32, hi: i32, fits: bool| if fits { v.clamp(lo, hi) } else { lo - gap };
    Placed {
        pos: PhysicalPosition::new(
            clamp(x, area.left + gap, area.right - gap - pw, fits_x),
            clamp(y, area.top + gap, area.bottom - gap - ph, fits_y),
        ),
        fits: fits_x && fits_y,
    }
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
/// Fob: give the mini window its fixed size and put it next to the tray icon
/// (above it for a bottom taskbar), clamped to the monitor. Only `show` calls this.
fn place(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    anchor: Option<(f64, f64, f64, f64)>,
) -> Result<(), String> {
    let anchor = {
        let state = app.state::<Mutex<CompanionState>>();
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if anchor.is_some() {
            state.anchor = anchor;
        }
        state.anchor
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
    record_placement(app, &screen, anchor, scale, size, &placed);
    // Position first, then size: the window is already on the target monitor
    // (and DPI) when it gets its size, so Windows does not rescale it.
    window.set_position(placed.pos).map_err(|e| e.to_string())?;
    window
        .set_size(tauri::Size::Physical(size))
        .map_err(|e| e.to_string())
}
/// Fob: the last placement's numbers, overwritten on every show, in the app
/// log folder (mini-placement.txt) so a misplaced window can be explained.
fn record_placement(
    app: &AppHandle,
    screen: &Screen,
    anchor: Option<(f64, f64, f64, f64)>,
    scale: f64,
    size: tauri::PhysicalSize<u32>,
    placed: &Placed,
) {
    let Ok(dir) = app.path().app_log_dir() else {
        return;
    };
    let text = format!(
        "scale {scale}\nmonitor {:?}\nwork {:?}\ntaskbar {:?}\ntray icon {:?}\n\
         panel {}x{} at {},{}\nfits work area: {}\n",
        screen.monitor,
        screen.work,
        screen.taskbar,
        anchor,
        size.width,
        size.height,
        placed.pos.x,
        placed.pos.y,
        placed.fits,
    );
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("mini-placement.txt"), text);
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
    match (window.label(), event) {
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
        assert_eq!(callers(concat!(" place", "(")), 2); // the fn and its one call in `show`
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
        for scale in [1.0, 1.25, 1.5] {
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
        for scale in [1.0, 1.25, 1.5] {
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
    fn too_tall_for_the_work_area_is_reported_not_hidden() {
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
        assert_eq!(p.pos.y, 0); // top visible; fits = false is recorded
    }
}
