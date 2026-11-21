//! Fob 0.7.9: start with the user's Windows session. One value,
//! `Fob`, under HKCU\Software\Microsoft\Windows\CurrentVersion\Run; per user,
//! so no administrator rights. Nothing else in the registry is touched.

pub fn command_line(exe: &std::path::Path) -> String {
    format!("\"{}\"", exe.display())
}

#[derive(Debug, PartialEq)]
pub enum Change {
    Write(String),
    Remove,
}
pub fn plan(enabled: bool, wanted: &str, current: Option<&str>) -> Option<Change> {
    match (enabled, current) {
        (true, Some(now)) if now == wanted => None,
        (true, _) => Some(Change::Write(wanted.to_owned())),
        (false, Some(_)) => Some(Change::Remove),
        (false, None) => None,
    }
}

pub fn sync(enabled: bool) {
    #[cfg(target_os = "windows")]
    {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let wanted = command_line(&exe);
        match plan(enabled, &wanted, run::read().as_deref()) {
            Some(Change::Write(cmd)) => run::write(&cmd),
            Some(Change::Remove) => run::remove(),
            None => {}
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = enabled;
}

#[cfg(target_os = "windows")]
mod run {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
    };
    const KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
    const NAME: windows::core::PCWSTR = w!("Fob");

    pub fn read() -> Option<String> {
        let mut size = 0u32;
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                KEY,
                NAME,
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut size),
            )
        }
        .ok()
        .ok()?;
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                KEY,
                NAME,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        }
        .ok()
        .ok()?;
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }
    pub fn write(cmd: &str) {
        let wide: Vec<u16> = cmd.encode_utf16().chain([0]).collect();
        let _ = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                KEY,
                NAME,
                REG_SZ.0,
                Some(wide.as_ptr().cast()),
                (wide.len() * 2) as u32,
            )
        };
    }
    pub fn remove() {
        let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, KEY, NAME) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_follows_the_setting() {
        let me = command_line(std::path::Path::new(r"C:\Apps\Fob\fob.exe"));
        assert_eq!(me, r#""C:\Apps\Fob\fob.exe""#);
        assert_eq!(plan(true, &me, None), Some(Change::Write(me.clone())));
        assert_eq!(plan(true, &me, Some(&me)), None);
        assert_eq!(
            plan(true, &me, Some(r#""D:\old\fob.exe""#)),
            Some(Change::Write(me.clone()))
        );
        assert_eq!(plan(false, &me, Some(&me)), Some(Change::Remove));
        assert_eq!(plan(false, &me, None), None);
    }
}
