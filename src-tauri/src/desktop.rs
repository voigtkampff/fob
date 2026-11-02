//! Settings and About windows. winperf has no native menu bar: settings,
//! about, collapse to tray and quit live in the app's own title bar menu and
//! the tray menu.
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
#[derive(Default)]
pub struct DesktopState(pub Mutex<String>);
fn show(app: &AppHandle, label: &str) -> Result<(), String> {
    crate::companion::hide(app);
    let window = app.get_webview_window(label).ok_or("Window unavailable")?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn open_settings(app: AppHandle, pane: Option<String>) -> Result<(), String> {
    if let Some(pane) = pane {
        if pane == "general" || pane == "appearance" {
            *app.state::<DesktopState>()
                .0
                .lock()
                .map_err(|e| e.to_string())? = pane.clone();
            app.emit_to("settings", "settings-pane", pane)
                .map_err(|e| e.to_string())?;
        }
    }
    show(&app, "settings")
}
#[tauri::command]
pub fn settings_pane(app: AppHandle) -> String {
    app.state::<DesktopState>()
        .0
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default()
}
#[tauri::command]
pub fn open_about(app: AppHandle, check: bool) -> Result<(), String> {
    show(&app, "about")?;
    if check {
        app.emit_to("about", "check-updates", ())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
