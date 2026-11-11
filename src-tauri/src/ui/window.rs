//! Window effects and customization
//!
//! Provides platform-specific window effects like transparency and vibrancy.

use tauri::WebviewWindow;

#[cfg(target_os = "windows")]
use window_vibrancy::apply_acrylic;
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

/// The main window's system backdrop. Only the Glassy theme lets it show
/// through the page (app.css); every other theme paints an opaque page over
/// it, so Acrylic's live blur of whatever is behind the window would be GPU
/// work nobody sees, done by DWM while the window is active (Acrylic turns
/// solid when it is not). Those themes get Mica, which samples the wallpaper
/// once, for the title bar and the moment before the page paints.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum MainBackdrop {
    Acrylic,
    Mica,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
impl MainBackdrop {
    pub fn for_page(glass: bool) -> Self {
        if glass {
            Self::Acrylic
        } else {
            Self::Mica
        }
    }
}

/// Applies Windows-specific window effects: Mica until the page says its
/// theme is Glassy (`set_main_backdrop`).
#[cfg(target_os = "windows")]
pub fn setup_window_effects(window: &WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    apply_main_backdrop(window, MainBackdrop::Mica)?;
    Ok(())
}

#[cfg(target_os = "windows")]
static MAIN_BACKDROP: std::sync::Mutex<Option<MainBackdrop>> = std::sync::Mutex::new(None);

/// Before Windows 11 there is no Mica; those windows keep Acrylic, as before.
#[cfg(target_os = "windows")]
pub fn apply_main_backdrop(
    window: &WebviewWindow,
    backdrop: MainBackdrop,
) -> Result<(), window_vibrancy::Error> {
    let mut current = MAIN_BACKDROP.lock().unwrap_or_else(|e| e.into_inner());
    if *current == Some(backdrop) {
        return Ok(());
    }
    let acrylic = || apply_acrylic(window, Some((0, 0, 25, 125)));
    match backdrop {
        MainBackdrop::Acrylic => acrylic()?,
        MainBackdrop::Mica => {
            if window_vibrancy::apply_mica(window, None).is_err() {
                acrylic()?;
            }
        }
    }
    *current = Some(backdrop);
    Ok(())
}

/// The main window's page applied a theme; `glass` is the Glassy theme.
#[tauri::command]
pub async fn set_main_backdrop(app: tauri::AppHandle, glass: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        let handle = app.clone();
        app.run_on_main_thread(move || {
            if let Some(window) = handle.get_webview_window("main") {
                if let Err(e) = apply_main_backdrop(&window, MainBackdrop::for_page(glass)) {
                    eprintln!("main backdrop: {e}");
                }
            }
        })
        .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, glass);
        Ok(())
    }
}

/// Fob: the mini window's glass: the Windows 11 system backdrop
/// (DWMWA_SYSTEMBACKDROP_TYPE), Acrylic by default. Windows draws a backdrop
/// in its inactive look whenever the window's frame is inactive, so the
/// window proc keeps the frame painted active (see `keep_frame_active`).
/// Before build 22523 window-vibrancy would fall back to the accent-policy
/// acrylic, which lags while dragging; those builds and Windows 10 keep plain
/// translucency. `FOB_MINI_BACKDROP=mica|tabbed` (hidden, for comparison)
/// picks Mica or Mica Alt instead.
#[cfg(target_os = "windows")]
pub fn setup_mini_glass(window: &WebviewWindow) {
    let build = sysinfo::System::kernel_version()
        .and_then(|v| v.split('.').next_back().and_then(|b| b.parse::<u32>().ok()));
    if !acrylic_supported(build) {
        return;
    }
    let kind = backdrop_kind(std::env::var("FOB_MINI_BACKDROP").ok().as_deref());
    let applied = match kind {
        Backdrop::Acrylic => apply_acrylic(window, Some((0, 0, 0, 0))),
        Backdrop::Mica => window_vibrancy::apply_mica(window, None),
        Backdrop::Tabbed => window_vibrancy::apply_tabbed(window, None),
    };
    if let Err(e) = applied {
        eprintln!("mini glass: {e}");
        return;
    }
    if let Ok(hwnd) = window.hwnd() {
        keep_frame_active(hwnd.0 as isize);
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, PartialEq, Clone, Copy)]
enum Backdrop {
    Acrylic,
    Mica,
    Tabbed,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn backdrop_kind(name: Option<&str>) -> Backdrop {
    match name.map(str::to_ascii_lowercase).as_deref() {
        Some("mica") => Backdrop::Mica,
        Some("tabbed") => Backdrop::Tabbed,
        _ => Backdrop::Acrylic,
    }
}

/// Subclass the mini window so WM_NCACTIVATE always paints the active frame
/// (wParam = TRUE), which keeps the system backdrop in its active look.
/// Focus itself is untouched: activation, WM_ACTIVATE and WM_KILLFOCUS still
/// happen, so tao's Focused(false) (auto-hide when unpinned) still fires.
#[cfg(target_os = "windows")]
fn keep_frame_active(hwnd: isize) {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::WM_NCACTIVATE;
    unsafe extern "system" fn proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if msg == WM_NCACTIVATE {
            // lParam -1: don't repaint the non-client area. The window has
            // no visible caption, and repainting it is what could draw the
            // hidden "Fob mini" title under the transparent page.
            let _ = lparam;
            return DefSubclassProc(hwnd, msg, WPARAM(1), LPARAM(-1));
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
    unsafe {
        if !SetWindowSubclass(HWND(hwnd as _), Some(proc), 0xF0B, 0).as_bool() {
            eprintln!("mini glass: SetWindowSubclass failed");
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn setup_mini_glass(_window: &WebviewWindow) {}

/// DWMWA_SYSTEMBACKDROP_TYPE exists from build 22523 (Windows 11 22H2).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn acrylic_supported(build: Option<u32>) -> bool {
    build.is_some_and(|b| b >= 22523)
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_glassy_page_gets_the_live_blur() {
        use super::MainBackdrop;
        assert_eq!(MainBackdrop::for_page(true), MainBackdrop::Acrylic);
        assert_eq!(MainBackdrop::for_page(false), MainBackdrop::Mica);
        // Glassy is the one theme whose page lets the backdrop show.
        let css = include_str!("../../../src/app.css");
        assert!(css.contains("[data-theme=\"glassy\"] body {\n  background: transparent;"));
        let theme = include_str!("../../../src/lib/stores/theme.ts");
        assert!(
            theme.contains("invoke(\"set_main_backdrop\", { glass: theme.name === \"glassy\" })")
        );
    }
    #[test]
    fn backdrop_defaults_to_acrylic() {
        use super::{backdrop_kind, Backdrop};
        assert_eq!(backdrop_kind(None), Backdrop::Acrylic);
        assert_eq!(backdrop_kind(Some("MICA")), Backdrop::Mica);
        assert_eq!(backdrop_kind(Some("tabbed")), Backdrop::Tabbed);
        assert_eq!(backdrop_kind(Some("junk")), Backdrop::Acrylic);
    }
    #[test]
    fn acrylic_only_on_windows_11() {
        assert!(super::acrylic_supported(Some(22631)));
        assert!(super::acrylic_supported(Some(22523)));
        assert!(
            !super::acrylic_supported(Some(22000)),
            "21H2: accent fallback lags"
        );
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
