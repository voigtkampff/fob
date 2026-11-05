//! Fob status bar (v0.7.0): a thin always-on-top row of readings, like a
//! game performance HUD. It reads the shared sampler only; nothing is
//! injected into other processes, so there is no FPS or frame time.
//!
//! Click-through by default. While Ctrl is held (polled with
//! GetAsyncKeyState every 50 ms, no keyboard hook) or "unlock" is on, it
//! takes the mouse so it can be dragged. A drag that ends near the top or
//! bottom of the work area snaps to that edge; the spot is saved per monitor
//! in overlay-position.json.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "hud";
/// Logical height of the bar.
pub const HEIGHT: f64 = 26.0;
/// A drag ending this close (logical px) to the top or bottom snaps there.
const SNAP: f64 = 24.0;
const POLL: Duration = Duration::from_millis(50);
/// Moves the app makes itself; a Moved event inside this window is not a drag.
const OWN_MOVE: Duration = Duration::from_millis(400);
/// A drag is over once the bar has been still this long.
const SETTLE: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Dock {
    #[default]
    Top,
    Bottom,
    Free,
}

/// Physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    fn right(&self) -> i32 {
        self.x + self.w
    }
    fn bottom(&self) -> i32 {
        self.y + self.h
    }
    fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
    fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// Where the bar was left: its monitor and its offset from that monitor's
/// work area, so it comes back to the same spot on the same screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub monitor: String,
    pub dx: i32,
    pub dy: i32,
}

/// A monitor: its name and work area (without the taskbar).
#[derive(Clone, Debug)]
pub struct Screen {
    pub name: String,
    pub work: Rect,
}

/// `r` moved fully inside `work` (a bar wider than the area keeps its left edge).
pub fn clamp(r: Rect, work: Rect) -> Rect {
    let x = r.x.min(work.right() - r.w).max(work.x);
    let y = r.y.min(work.bottom() - r.h).max(work.y);
    Rect { x, y, ..r }
}

/// The bar after a drag: inside the work area, and on the top or bottom edge
/// when it ended within `threshold` px of it.
pub fn snap(r: Rect, work: Rect, threshold: i32) -> (Rect, Dock) {
    let r = clamp(r, work);
    if r.y - work.y <= threshold {
        (Rect { y: work.y, ..r }, Dock::Top)
    } else if work.bottom() - r.bottom() <= threshold {
        (
            Rect {
                y: work.bottom() - r.h,
                ..r
            },
            Dock::Bottom,
        )
    } else {
        (r, Dock::Free)
    }
}

/// Where the bar goes: the saved monitor (or the primary one when that
/// monitor is gone) at the saved offset, on the docked edge, inside the
/// work area. Nothing saved: centred at the top.
pub fn restore(
    saved: Option<&Saved>,
    screens: &[Screen],
    primary: usize,
    size: (i32, i32),
    dock: Dock,
) -> Option<(Rect, usize)> {
    let found = saved.and_then(|s| screens.iter().position(|m| m.name == s.monitor));
    let index = found.or(Some(primary).filter(|&p| p < screens.len()))?;
    let work = screens[index].work;
    let (w, h) = size;
    let (x, y) = match (saved, found) {
        (Some(s), Some(_)) => (work.x + s.dx, work.y + s.dy),
        _ => (work.x + (work.w - w) / 2, work.y),
    };
    let r = clamp(Rect { x, y, w, h }, work);
    let r = match dock {
        Dock::Top => Rect { y: work.y, ..r },
        Dock::Bottom => Rect {
            y: work.bottom() - h,
            ..r
        },
        Dock::Free => r,
    };
    Some((r, index))
}

/// The screen the bar's centre is on, or the one it overlaps most.
pub fn screen_of(r: Rect, screens: &[Screen]) -> Option<usize> {
    screens
        .iter()
        .position(|m| m.work.contains(r.center()))
        .or_else(|| {
            let overlap = |m: &Screen| {
                let w = (r.right().min(m.work.right()) - r.x.max(m.work.x)).max(0);
                let h = (r.bottom().min(m.work.bottom()) - r.y.max(m.work.y)).max(0);
                i64::from(w) * i64::from(h)
            };
            (0..screens.len()).max_by_key(|&i| overlap(&screens[i]))
        })
}

/// The overlay settings (behavior.overlay*, validated in preferences.rs).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Config {
    pub enabled: bool,
    pub dock: Dock,
    pub unlocked: bool,
}
impl Config {
    pub fn from_prefs(config: &Value) -> Self {
        let b = &config["behavior"];
        Self {
            enabled: b["overlayEnabled"].as_bool().unwrap_or(false),
            dock: serde_json::from_value(b["overlayDock"].clone()).unwrap_or_default(),
            unlocked: b["overlayUnlocked"].as_bool().unwrap_or(false),
        }
    }
}

#[derive(Default)]
pub struct OverlayState {
    config: Config,
    /// Logical width the page measured for its row.
    width: Option<f64>,
    own_move: Option<Instant>,
    moved: Option<Instant>,
    saving: bool,
    interactive: bool,
}
pub type Shared = Mutex<OverlayState>;
static POLLING: AtomicBool = AtomicBool::new(false);

fn screens(window: &WebviewWindow) -> (Vec<Screen>, usize) {
    let monitors = window.available_monitors().unwrap_or_default();
    let primary = window.primary_monitor().ok().flatten();
    let list: Vec<Screen> = monitors
        .iter()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            let full = Rect {
                x: p.x,
                y: p.y,
                w: s.width as i32,
                h: s.height as i32,
            };
            Screen {
                name: m.name().cloned().unwrap_or_default(),
                work: work_area(full),
            }
        })
        .collect();
    let index = primary
        .and_then(|p| list.iter().position(|s| Some(&s.name) == p.name()))
        .unwrap_or(0);
    (list, index)
}

#[cfg(windows)]
fn work_area(full: Rect) -> Rect {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    let (x, y) = full.center();
    unsafe {
        let hmon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut info).as_bool() {
            let r = info.rcWork;
            return Rect {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            };
        }
    }
    full
}
#[cfg(not(windows))]
fn work_area(full: Rect) -> Rect {
    full
}

fn position_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(
        app.path()
            .app_config_dir()
            .ok()?
            .join("overlay-position.json"),
    )
}
fn load(app: &AppHandle) -> Option<Saved> {
    serde_json::from_slice(&std::fs::read(position_file(app)?).ok()?).ok()
}
fn save(app: &AppHandle, saved: &Saved) {
    if let (Some(path), Ok(bytes)) = (position_file(app), serde_json::to_vec_pretty(saved)) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, bytes);
    }
}

fn window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("/?view=overlay".into()))
        .title("Fob status bar")
        .inner_size(420.0, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        // Not resizable on Windows: a resizable undecorated window gets a
        // sizing border from the hit-test, so with the mouse let in (Ctrl)
        // its top and bottom edges dragged the bar taller. GTK won't shrink
        // a fixed-size window below 200 px, so Linux keeps it resizable;
        // min = max (place) stops any resize there.
        .resizable(cfg!(target_os = "linux"))
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    no_activate(&w);
    Ok(w)
}

/// Clicking or dragging the bar never takes focus from the game or app
/// under it (WS_EX_NOACTIVATE on our own window).
#[cfg(windows)]
fn no_activate(w: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    if let Ok(handle) = w.hwnd() {
        // tauri's HWND is another `windows` crate version's type.
        let hwnd = HWND(handle.0 as _);
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                style | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize,
            );
        }
    }
}
#[cfg(not(windows))]
fn no_activate(_: &WebviewWindow) {}

/// Puts the bar where `restore` says, at its measured width.
fn place(app: &AppHandle, w: &WebviewWindow) {
    let (config, width) = match app.state::<Shared>().lock() {
        Ok(s) => (s.config, s.width.unwrap_or(420.0)),
        Err(_) => return,
    };
    let (list, primary) = screens(w);
    let saved = load(app);
    let scale = w.scale_factor().unwrap_or(1.0);
    let size = (
        (width * scale).round() as i32,
        (HEIGHT * scale).round() as i32,
    );
    let Some((r, _)) = restore(saved.as_ref(), &list, primary, size, config.dock) else {
        return;
    };
    if let Ok(mut s) = app.state::<Shared>().lock() {
        s.own_move = Some(Instant::now());
    }
    // Min = max = the size: GTK keeps a fixed-size window at its first size
    // otherwise.
    let size = tauri::PhysicalSize::new(r.w as u32, r.h as u32);
    let _ = w.set_min_size(Some(size));
    let _ = w.set_max_size(Some(size));
    let _ = w.set_size(size);
    let _ = w.set_position(tauri::PhysicalPosition::new(r.x, r.y));
}

/// Mouse goes to the bar (drag) or through it (default).
fn set_interactive(app: &AppHandle, w: &WebviewWindow, on: bool) {
    let changed = app
        .state::<Shared>()
        .lock()
        .map(|mut s| std::mem::replace(&mut s.interactive, on) != on)
        .unwrap_or(false);
    if changed {
        let _ = w.set_ignore_cursor_events(!on);
        let _ = app.emit_to(LABEL, "overlay-interactive", on);
    }
}

/// Settings changed or loaded: show, hide or re-place the bar.
pub fn sync(app: &AppHandle, prefs: &Value) {
    let config = Config::from_prefs(prefs);
    if let Ok(mut s) = app.state::<Shared>().lock() {
        s.config = config;
    }
    crate::companion::set_overlay_checked(app, config.enabled);
    if !config.enabled {
        if let Some(w) = app.get_webview_window(LABEL) {
            let _ = w.hide();
        }
        return;
    }
    let Ok(w) = window(app) else { return };
    place(app, &w);
    let _ = w.show();
    // Showing can reset the extended style on some systems; force a resend.
    if let Ok(mut s) = app.state::<Shared>().lock() {
        s.interactive = !config.unlocked;
    }
    set_interactive(app, &w, config.unlocked);
    poll(app.clone());
}

/// Ctrl watcher, only while the bar is shown; also brings the bar back
/// into view when its monitor goes away.
fn poll(app: AppHandle) {
    if POLLING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("fob-hud".into())
        .spawn(move || {
            let mut tick = 0u32;
            loop {
                let Some(w) = app.get_webview_window(LABEL) else {
                    break;
                };
                let config = match app.state::<Shared>().lock() {
                    Ok(s) => s.config,
                    Err(_) => break,
                };
                if !config.enabled || !w.is_visible().unwrap_or(false) {
                    break;
                }
                set_interactive(&app, &w, config.unlocked || ctrl_down());
                tick += 1;
                if tick % 40 == 0 {
                    // Every 2 s: off every monitor (one was unplugged)? Re-place.
                    if let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) {
                        let r = Rect {
                            x: p.x,
                            y: p.y,
                            w: s.width as i32,
                            h: s.height as i32,
                        };
                        let (list, _) = screens(&w);
                        if !list.iter().any(|m| m.work.contains(r.center())) {
                            place(&app, &w);
                        }
                    }
                }
                std::thread::sleep(POLL);
            }
            POLLING.store(false, Ordering::SeqCst);
        });
}

#[cfg(windows)]
fn ctrl_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
    unsafe { GetAsyncKeyState(i32::from(VK_CONTROL.0)) < 0 }
}
#[cfg(not(windows))]
fn ctrl_down() -> bool {
    false
}

/// The bar moved: unless the app moved it, wait for the drag to settle,
/// then snap, save and remember the dock.
pub fn moved(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let start = {
        let Ok(mut s) = shared.lock() else {
            return;
        };
        if s.own_move.is_some_and(|t| t.elapsed() < OWN_MOVE) {
            return;
        }
        s.moved = Some(Instant::now());
        !std::mem::replace(&mut s.saving, true)
    };
    if !start {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let still = app
                .state::<Shared>()
                .lock()
                .map(|s| s.moved.is_none_or(|t| t.elapsed() >= SETTLE))
                .unwrap_or(true);
            if still {
                break;
            }
        }
        if let Ok(mut s) = app.state::<Shared>().lock() {
            s.saving = false;
        }
        finish_drag(&app);
    });
}

fn finish_drag(app: &AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else {
        return;
    };
    let (Ok(p), Ok(sz)) = (w.outer_position(), w.outer_size()) else {
        return;
    };
    let r = Rect {
        x: p.x,
        y: p.y,
        w: sz.width as i32,
        h: sz.height as i32,
    };
    let (list, _) = screens(&w);
    let Some(i) = screen_of(r, &list) else { return };
    let work = list[i].work;
    let scale = w.scale_factor().unwrap_or(1.0);
    let (r, dock) = snap(r, work, (SNAP * scale).round() as i32);
    save(
        app,
        &Saved {
            monitor: list[i].name.clone(),
            dx: r.x - work.x,
            dy: r.y - work.y,
        },
    );
    let old = app.state::<Shared>().lock().map(|s| s.config.dock).ok();
    if old != Some(dock) {
        // sync() re-places the bar from the file just saved.
        let patch = serde_json::json!({ "behavior": { "overlayDock": dock } });
        let _ = crate::preferences::apply_patch(app, &patch);
    } else {
        place(app, &w);
    }
}

/// Toggles the bar from the tray.
pub fn toggle(app: &AppHandle) {
    let on = app
        .state::<Shared>()
        .lock()
        .map(|s| s.config.enabled)
        .unwrap_or(false);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let patch = serde_json::json!({ "behavior": { "overlayEnabled": !on } });
        let _ = crate::preferences::apply_patch(&app, &patch);
    });
}

pub fn enabled(app: &AppHandle) -> bool {
    app.state::<Shared>()
        .lock()
        .map(|s| s.config.enabled)
        .unwrap_or(false)
}

/// Whether the bar takes the mouse now (the page asks once it listens).
#[tauri::command]
pub async fn overlay_interactive(app: AppHandle) -> bool {
    app.state::<Shared>().lock().is_ok_and(|s| s.interactive)
}

/// The page measured its row: size the bar to it (logical px).
#[tauri::command]
pub async fn overlay_fit(app: AppHandle, width: f64) -> Result<(), String> {
    if !(40.0..=4000.0).contains(&width) {
        return Err("width out of range".into());
    }
    let w = app.get_webview_window(LABEL).ok_or("no status bar")?;
    if let Ok(mut s) = app.state::<Shared>().lock() {
        if s.width.is_some_and(|old| (old - width).abs() < 1.0) {
            return Ok(());
        }
        s.width = Some(width.ceil());
    }
    place(&app, &w);
    Ok(())
}

/// The readings the bar can show, from the shared sampler's last frame.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub cpu: Option<f64>,
    pub cpu_temp: Option<f64>,
    pub gpu: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub ram_used: Option<u64>,
    pub ram_total: Option<u64>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub disk_read: Option<u64>,
    pub disk_write: Option<u64>,
    pub net_rx: Option<u64>,
    pub net_tx: Option<u64>,
    pub gpu_power: Option<f64>,
    pub paused: bool,
    /// The last two minutes of samples, for the optional charts.
    pub history: Vec<crate::sampling::Sample>,
}

#[tauri::command]
pub async fn overlay_reading(app: AppHandle) -> Result<Reading, String> {
    let snap = crate::sampling::read(app).await?;
    let f = &snap.frame;
    let last = snap.history.last();
    let gpu = f.gpus.iter().max_by_key(|g| g.memory_total.unwrap_or(0));
    Ok(Reading {
        cpu: last.and_then(|s| s.cpu),
        cpu_temp: last.and_then(|s| s.cpu_temp),
        gpu: gpu.and_then(|g| g.utilization),
        gpu_temp: last
            .and_then(|s| s.gpu_temp)
            .or(gpu.and_then(|g| g.temperature)),
        ram_used: Some(f.stats.memory_used),
        ram_total: Some(f.stats.memory_total),
        vram_used: gpu.and_then(|g| g.memory_used),
        vram_total: gpu.and_then(|g| g.memory_total),
        disk_read: f.stats.disk_read_bytes,
        disk_write: f.stats.disk_write_bytes,
        net_rx: f.stats.network_rx_bytes,
        net_tx: f.stats.network_tx_bytes,
        gpu_power: gpu.and_then(|g| g.power_watts),
        paused: snap.paused,
        history: snap.history.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const WORK: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1040,
    };
    fn bar(x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            w: 400,
            h: 26,
        }
    }
    fn screens() -> Vec<Screen> {
        vec![
            Screen {
                name: r"\\.\DISPLAY1".into(),
                work: WORK,
            },
            Screen {
                name: r"\\.\DISPLAY2".into(),
                work: Rect {
                    x: 1920,
                    y: 0,
                    w: 2560,
                    h: 1400,
                },
            },
        ]
    }

    #[test]
    fn a_drag_near_an_edge_snaps_to_it() {
        assert_eq!(snap(bar(300, 20), WORK, 24), (bar(300, 0), Dock::Top));
        assert_eq!(
            snap(bar(300, 1000), WORK, 24),
            (bar(300, 1014), Dock::Bottom)
        );
        assert_eq!(snap(bar(300, 500), WORK, 24), (bar(300, 500), Dock::Free));
    }

    #[test]
    fn a_drag_past_the_work_area_is_pulled_back_in() {
        assert_eq!(snap(bar(1800, -40), WORK, 24), (bar(1520, 0), Dock::Top));
        assert_eq!(snap(bar(-90, 2000), WORK, 24), (bar(0, 1014), Dock::Bottom));
        assert_eq!(
            clamp(
                Rect {
                    w: 3000,
                    ..bar(50, 50)
                },
                WORK
            )
            .x,
            0
        );
    }

    #[test]
    fn first_start_is_centred_at_the_top_of_the_primary_screen() {
        let (r, i) = restore(None, &screens(), 0, (400, 26), Dock::Top).unwrap();
        assert_eq!((r, i), (bar(760, 0), 0));
    }

    #[test]
    fn the_saved_spot_comes_back_on_its_monitor() {
        let saved = Saved {
            monitor: r"\\.\DISPLAY2".into(),
            dx: 100,
            dy: 600,
        };
        let s = screens();
        let (r, i) = restore(Some(&saved), &s, 0, (400, 26), Dock::Free).unwrap();
        assert_eq!((r, i), (bar(2020, 600), 1));
        // The dock preset wins over the saved height.
        let (r, _) = restore(Some(&saved), &s, 0, (400, 26), Dock::Bottom).unwrap();
        assert_eq!(r, bar(2020, 1374));
    }

    #[test]
    fn a_missing_monitor_falls_back_to_the_primary_work_area() {
        let saved = Saved {
            monitor: r"\\.\DISPLAY3".into(),
            dx: 3000,
            dy: 900,
        };
        let (r, i) = restore(Some(&saved), &screens(), 0, (400, 26), Dock::Free).unwrap();
        assert_eq!(i, 0);
        assert!(r.x >= 0 && r.right() <= 1920 && r.y >= 0 && r.bottom() <= 1040);
        assert!(restore(None, &[], 0, (400, 26), Dock::Top).is_none());
    }

    #[test]
    fn the_screen_is_the_one_under_the_bar() {
        assert_eq!(screen_of(bar(2500, 300), &screens()), Some(1));
        assert_eq!(screen_of(bar(1700, 300), &screens()), Some(0));
        assert_eq!(screen_of(bar(1800, 300), &screens()), Some(1));
    }

    #[test]
    fn settings_read_with_safe_defaults() {
        let c = Config::from_prefs(&serde_json::json!({ "behavior": {} }));
        assert_eq!(c, Config::default());
        assert!(!c.enabled);
        let c = Config::from_prefs(&serde_json::json!({ "behavior": {
            "overlayEnabled": true, "overlayDock": "bottom", "overlayUnlocked": true } }));
        assert_eq!(
            c,
            Config {
                enabled: true,
                dock: Dock::Bottom,
                unlocked: true
            }
        );
        let c = Config::from_prefs(&serde_json::json!({ "behavior": { "overlayDock": "left" } }));
        assert_eq!(c.dock, Dock::Top);
    }
}
