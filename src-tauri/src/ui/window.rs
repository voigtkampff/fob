//! Window effects and customization
//!
//! Provides platform-specific window effects like transparency and vibrancy.

use tauri::WebviewWindow;

#[cfg(target_os = "windows")]
use window_vibrancy::apply_acrylic;
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

/// Applies Windows-specific window effects
#[cfg(target_os = "windows")]
pub fn setup_window_effects(window: &WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    apply_acrylic(window, Some((0, 0, 25, 125)))?;
    Ok(())
}

/// Fob: the mini window's glass. Windows 11 gets the native Acrylic backdrop
/// (the desktop blurred behind the transparent webview); the CSS tint, edge
/// light and shadow sit on top (app.css). Windows 10's accent-policy acrylic
/// lags while dragging, so it keeps plain translucency.
#[cfg(target_os = "windows")]
pub fn setup_mini_glass(window: &WebviewWindow) {
    let build = sysinfo::System::kernel_version()
        .and_then(|v| v.split('.').next_back().and_then(|b| b.parse::<u32>().ok()));
    if !acrylic_supported(build) {
        return;
    }
    // Tint (0,0,0,0): the theme tint is drawn by CSS so it follows the theme.
    if let Err(e) = apply_acrylic(window, Some((0, 0, 0, 0))) {
        eprintln!("mini glass: {e}");
    }
}

#[cfg(not(target_os = "windows"))]
pub fn setup_mini_glass(_window: &WebviewWindow) {}

/// Windows 11 starts at build 22000.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn acrylic_supported(build: Option<u32>) -> bool {
    build.is_some_and(|b| b >= 22000)
}

#[cfg(test)]
mod tests {
    #[test]
    fn acrylic_only_on_windows_11() {
        assert!(super::acrylic_supported(Some(22631)));
        assert!(super::acrylic_supported(Some(22000)));
        assert!(!super::acrylic_supported(Some(19045)));
        assert!(!super::acrylic_supported(None));
    }
}

/// Applies macOS-specific window effects
#[cfg(target_os = "macos")]
pub fn setup_window_effects(window: &WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    apply_vibrancy(
        window,
        NSVisualEffectMaterial::HudWindow,
        Some(NSVisualEffectState::Active),
        None,
    )?;
    Ok(())
}

/// No-op for platforms without specific window effects
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn setup_window_effects(_window: &WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
