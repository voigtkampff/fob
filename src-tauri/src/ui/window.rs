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

/// Fob: the mini window's outline: a squircle (|x|^4 + |y|^4 = 1 per corner,
/// the same curve as CSS `corner-shape: squircle` in app.css). Windows'
/// own rounding is turned off and the window region is clipped to this
/// outline, so the acrylic blur ends exactly at the CSS shell. A region has
/// no anti-aliasing and drops the DWM shadow; the shell draws a 1px edge.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const MINI_RADIUS_CSS: f64 = 22.0;

/// Outline points (physical px) for a `w` x `h` box with corner radius `r`.
pub fn squircle_points(w: f64, h: f64, r: f64) -> Vec<(i32, i32)> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    const STEPS: usize = 24;
    let corners = [
        (w - r, r, 1.0, -1.0, 0.0),
        (r, r, -1.0, -1.0, 1.0),
        (r, h - r, -1.0, 1.0, 2.0),
        (w - r, h - r, 1.0, 1.0, 3.0),
    ];
    let mut pts = Vec::with_capacity(4 * (STEPS + 1));
    for (cx, cy, _, _, q) in corners {
        for i in 0..=STEPS {
            let t = (q + i as f64 / STEPS as f64) * std::f64::consts::FRAC_PI_2;
            let (c, s) = (t.cos(), -t.sin());
            let x = cx + r * c.signum() * c.abs().sqrt();
            let y = cy + r * s.signum() * s.abs().sqrt();
            pts.push((x.round() as i32, y.round() as i32));
        }
    }
    pts.dedup();
    pts
}

/// Clip the mini window to the squircle and turn off DWM rounding.
#[cfg(target_os = "windows")]
pub fn apply_mini_shape(hwnd: isize, scale: f64) {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };
    use windows::Win32::Graphics::Gdi::{CreatePolygonRgn, SetWindowRgn, WINDING};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let hwnd = HWND(hwnd as _);
    unsafe {
        let pref = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&pref) as u32,
        );
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return;
        }
        let (w, h) = (
            (rect.right - rect.left) as f64,
            (rect.bottom - rect.top) as f64,
        );
        let pts: Vec<POINT> = squircle_points(w, h, MINI_RADIUS_CSS * scale)
            .into_iter()
            .map(|(x, y)| POINT { x, y })
            .collect();
        let rgn = CreatePolygonRgn(&pts, WINDING);
        // The system owns the region after SetWindowRgn succeeds.
        let _ = SetWindowRgn(hwnd, Some(rgn), true);
    }
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
    fn squircle_stays_inside_the_box_and_touches_each_edge() {
        let pts = super::squircle_points(360.0, 652.0, 33.0);
        assert!(pts
            .iter()
            .all(|&(x, y)| (0..=360).contains(&x) && (0..=652).contains(&y)));
        for edge in [
            |p: &(i32, i32)| p.0 == 0,
            |p: &(i32, i32)| p.0 == 360,
            |p: &(i32, i32)| p.1 == 0,
            |p: &(i32, i32)| p.1 == 652,
        ] {
            assert!(pts.iter().any(edge));
        }
        // The corner itself is cut: a squircle is not a square.
        assert!(!pts.contains(&(0, 0)));
        // Fuller than a circle: at 45 degrees it sits further out than r(1-cos45).
        let r = 33.0_f64;
        let circle_inset = r - r * std::f64::consts::FRAC_1_SQRT_2;
        let sq_inset = r - r * 0.5_f64.sqrt().sqrt();
        assert!(sq_inset < circle_inset);
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
