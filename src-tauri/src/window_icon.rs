//! Fob: sharp window icons on Windows.
//!
//! Tauri 2.1 builds its default window icon from the FIRST frame of
//! icons/icon.ico (the 16 px one) and tao sends it only as WM_SETICON
//! ICON_SMALL; ICON_BIG is never set and the window class has no icon. So the
//! taskbar and Alt-Tab stretched a 16 px bitmap to 24-48 px: the blurry icon.
//! Here every window gets both icons loaded from the exe's own icon group
//! (resource 32512, all sizes 16-256) at the sizes Windows asks for at the
//! window's current DPI, and again whenever the window moves to another DPI.

pub fn apply(window: &tauri::Window) {
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = window.hwnd() {
        set_icons(hwnd.0 as isize);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}
pub fn apply_all(app: &tauri::AppHandle) {
    use tauri::Manager;
    for _window in app.webview_windows().values() {
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = _window.hwnd() {
            set_icons(hwnd.0 as isize);
        }
    }
}
#[cfg(target_os = "windows")]
fn set_icons(raw: isize) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, LoadImageW, SendMessageW, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON,
        LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, WM_SETICON,
    };
    /// The icon group tauri-build embeds (IDI_APPLICATION's number).
    const APP_ICON: u16 = 32512;
    let hwnd = HWND(raw as _);
    unsafe {
        let Ok(module) = GetModuleHandleW(PCWSTR::null()) else {
            return;
        };
        let dpi = GetDpiForWindow(hwnd).max(96);
        for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
            let size = GetSystemMetricsForDpi(metric, dpi);
            let Ok(icon) = LoadImageW(
                Some(module.into()),
                PCWSTR(APP_ICON as usize as *const u16),
                IMAGE_ICON,
                size,
                size,
                LR_DEFAULTCOLOR,
            ) else {
                continue;
            };
            let old = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(kind as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
            // Free the icon this replaced if it was ours (a DPI change);
            // tao owns and frees the one it set at startup.
            if old.0 != 0 && ours().replace(hwnd.0 as isize, kind, old.0) {
                let _ = DestroyIcon(HICON(old.0 as _));
            }
            ours().insert(hwnd.0 as isize, kind, icon.0 as isize);
        }
    }
}

#[cfg(target_os = "windows")]
fn ours() -> std::sync::MutexGuard<'static, Owned> {
    static OWNED: std::sync::Mutex<Owned> = std::sync::Mutex::new(Owned(Vec::new()));
    OWNED.lock().unwrap_or_else(|e| e.into_inner())
}
/// Icons this module created: (window, ICON_BIG/SMALL, HICON).
#[cfg(target_os = "windows")]
struct Owned(Vec<(isize, u32, isize)>);
#[cfg(target_os = "windows")]
impl Owned {
    /// Was `icon` (the one just replaced on `hwnd`) created here?
    fn replace(&mut self, hwnd: isize, kind: u32, icon: isize) -> bool {
        let before = self.0.len();
        self.0.retain(|e| *e != (hwnd, kind, icon));
        self.0.len() != before
    }
    fn insert(&mut self, hwnd: isize, kind: u32, icon: isize) {
        self.0.push((hwnd, kind, icon));
    }
}

#[cfg(test)]
mod tests {
    /// Every frame Windows asks for at 100-250 % (title bar, taskbar,
    /// Alt-Tab, Start, Explorer), each a 32-bit image of its own.
    #[test]
    fn icon_ico_has_every_windows_size() {
        let ico = include_bytes!("../icons/icon.ico");
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        let mut sizes: Vec<u32> = (0..count)
            .map(|i| {
                let e = &ico[6 + 16 * i..22 + 16 * i];
                assert_eq!(u16::from_le_bytes([e[6], e[7]]), 32, "32-bit with alpha");
                if e[0] == 0 {
                    256
                } else {
                    e[0] as u32
                }
            })
            .collect();
        sizes.sort();
        assert_eq!(
            sizes,
            [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256]
        );
    }
    #[test]
    fn every_window_gets_its_icons_at_start_and_on_dpi_changes() {
        let main = include_str!("main.rs");
        assert!(main.contains("window_icon::apply_all(app.handle())"));
        let companion = include_str!("companion.rs");
        assert!(companion.contains("WindowEvent::ScaleFactorChanged { .. } = event"));
        assert!(companion.contains("crate::window_icon::apply(window)"));
    }
}
