#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! NeoHtop - A modern system monitor built with Tauri
//!
//! This is the main entry point for the application. It sets up the Tauri
//! application, initializes plugins, and configures window effects.

mod autostart;
mod boot;
mod commands;
mod companion;
mod desktop;
mod disk_health;
mod gpu;
mod hotkey;
mod i18n;
mod main_window;
mod mft_records;
mod mft_wire;
mod monitoring;
mod native_icons;
mod overlay;
mod preferences;
mod sampling;
mod sensors;
mod space;
mod space_helper;
mod space_mft;
mod state;
mod ui;
mod window_icon;

use state::AppState;
use tauri::Manager;

/// Main entry point for the application
///
/// # Panics
///
/// Will panic if:
/// - Unable to create the main window
/// - Failed to apply window effects
/// - Failed to initialize the application state
fn main() {
    // winperf: elevated one-shot sensor setup/removal (relaunched via UAC).
    if let Some(code) = sensors::setup::handle_cli() {
        std::process::exit(code);
    }
    #[cfg(target_os = "linux")]
    if std::env::var("XDG_SESSION_TYPE").unwrap_or_default() == "wayland" {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            ui::setup_window_effects(&window).expect("Failed to apply window effects");
            if let Some(mini) = app.get_webview_window("overview") {
                ui::setup_mini_glass(&mini);
            }
            companion::native_frame(app.handle());
            window_icon::apply_all(app.handle());
            sampling::sample_in_background(app.handle().clone());
            let login = preferences::startup(app.handle()).launch_at_login;
            std::thread::spawn(move || autostart::sync(login));
            // winperf: tray-only start (default) or the main window.
            companion::launch(app.handle())?;
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(main_window::plugin())
        .plugin(hotkey::plugin())
        .manage(hotkey::HotkeyState::default())
        .manage(AppState::new())
        .manage(space::SpaceState::default())
        .manage(disk_health::DiskHealthState::default())
        .manage(preferences::Preferences::default())
        .manage(desktop::DesktopState::default())
        .manage(overlay::Shared::default())
        .manage(std::sync::Mutex::new(companion::CompanionState::default()))
        .on_window_event(companion::window_event)
        .invoke_handler(tauri::generate_handler![
            ui::window::set_main_backdrop,
            preferences::read_preferences,
            preferences::update_preferences,
            desktop::open_settings,
            desktop::settings_pane,
            desktop::open_about,
            commands::get_processes,
            commands::get_process_icons,
            commands::kill_process,
            sampling::get_overview,
            sampling::set_monitor_paused,
            sampling::set_monitor_interval,
            companion::configure_companion,
            companion::show_companion,
            companion::toggle_companion,
            companion::hide_companion,
            companion::get_companion_pinned,
            companion::reset_mini_position,
            companion::mini_drag_begin,
            companion::get_mini_position_saved,
            companion::set_companion_pinned,
            companion::collapse_to_tray,
            companion::open_main_window,
            companion::quit_app,
            sensors::setup::sensor_setup_status,
            sensors::setup::sensor_setup,
            commands::disk_health,
            space::space_drives,
            space::space_scan,
            space::space_stop,
            space::space_status,
            space::space_result,
            space::space_reveal,
            space::space_folded,
            hotkey::get_hotkey,
            hotkey::set_hotkey,
            hotkey::suspend_hotkey,
            overlay::overlay_fit,
            overlay::overlay_interactive,
            overlay::overlay_reading,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                // Reactivating the app must not steal focus from Settings or About.
                // Restore the monitor when all document windows are hidden instead.
                let auxiliary_visible = ["settings", "about"].iter().any(|label| {
                    app.get_webview_window(label)
                        .is_some_and(|window| window.is_visible().unwrap_or(false))
                });
                if !has_visible_windows || !auxiliary_visible {
                    let _ = companion::open_main(app);
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
