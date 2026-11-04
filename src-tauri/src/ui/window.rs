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

/// Fob: the mini window's glass on Windows 11: the accent-policy acrylic
/// (SetWindowCompositionAttribute, ACCENT_ENABLE_ACRYLICBLURBEHIND). Unlike
/// the DWM system backdrop (DWMWA_SYSTEMBACKDROP_TYPE), which Windows swaps
/// for a flat fallback whenever the window is inactive, the accent blur stays
/// when another window has focus. Known cost: on some builds it lags while
/// the window is dragged. Windows 10 keeps plain translucency (lags worse).
/// The tint is drawn by CSS (theme + frost setting), so the accent tint is
/// almost clear.
#[cfg(target_os = "windows")]
pub fn setup_mini_glass(window: &WebviewWindow) {
    let build = sysinfo::System::kernel_version()
        .and_then(|v| v.split('.').next_back().and_then(|b| b.parse::<u32>().ok()));
    if !acrylic_supported(build) {
        return;
    }
    let Ok(hwnd) = window.hwnd() else { return };
    if let Err(e) = accent_acrylic(hwnd.0 as isize, 0x0100_0000) {
        eprintln!("mini glass: {e}");
    }
}

#[cfg(target_os = "windows")]
fn accent_acrylic(hwnd: isize, abgr: u32) -> Result<(), String> {
    use windows::core::s;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
    #[repr(C)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        gradient: u32,
        animation: u32,
    }
    #[repr(C)]
    struct CompositionData {
        attribute: u32,
        data: *mut std::ffi::c_void,
        size: usize,
    }
    const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
    const WCA_ACCENT_POLICY: u32 = 19;
    type SetWca = unsafe extern "system" fn(isize, *mut CompositionData) -> i32;
    unsafe {
        let user32 = LoadLibraryA(s!("user32.dll")).map_err(|e| e.to_string())?;
        let f = GetProcAddress(user32, s!("SetWindowCompositionAttribute"))
            .ok_or("SetWindowCompositionAttribute missing")?;
        let f: SetWca = std::mem::transmute(f);
        let mut policy = AccentPolicy {
            state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
            flags: 2,
            gradient: abgr,
            animation: 0,
        };
        let mut data = CompositionData {
            attribute: WCA_ACCENT_POLICY,
            data: &mut policy as *mut _ as *mut _,
            size: std::mem::size_of::<AccentPolicy>(),
        };
        if f(hwnd, &mut data) == 0 {
            return Err("SetWindowCompositionAttribute failed".into());
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn setup_mini_glass(_window: &WebviewWindow) {}

/// Fob: the mini window's native frame. Windows rounds the corners (anti-
/// aliased, with its shadow); the squircle is only inside (app.css). The
/// caption bits that tao keeps on an undecorated window (WS_CAPTION,
/// system menu, min/max boxes) are stripped: with a transparent client
/// area DWM painted that hidden caption ("Fob mini" and its buttons) under
/// the page. WS_THICKFRAME stays; it is what gives the window its shadow.
/// tao rewrites styles when some flags change, so this runs again on
/// focus and resize.
#[cfg(target_os = "windows")]
pub fn apply_mini_frame(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows::Win32::Graphics::Gdi::SetWindowRgn;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION, WS_MAXIMIZEBOX,
        WS_MINIMIZEBOX, WS_SYSMENU,
    };
    let hwnd = HWND(hwnd as _);
    unsafe {
        let pref = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&pref) as u32,
        );
        let _ = SetWindowRgn(hwnd, None, true);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let strip = (WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) as isize;
        let next = strip_caption(style, strip);
        if next != style {
            SetWindowLongPtrW(hwnd, GWL_STYLE, next);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

/// WS_CAPTION is two bits (WS_BORDER | WS_DLGFRAME); clear all of them.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn strip_caption(style: isize, strip: isize) -> isize {
    style & !strip
}

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
    #[test]
    fn caption_bits_are_cleared_and_the_frame_kept() {
        let (caption, sysmenu, thick, visible) =
            (0x00C0_0000, 0x0008_0000, 0x0004_0000, 0x1000_0000);
        let style = caption | sysmenu | thick | visible;
        let out = super::strip_caption(style, caption | sysmenu);
        assert_eq!(out & caption, 0);
        assert_eq!(out & sysmenu, 0);
        assert_eq!(out & (thick | visible), thick | visible);
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
