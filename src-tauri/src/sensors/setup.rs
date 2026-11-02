//! One-time setup of the read-only sensor helper, only after the user agrees
//! in the UI. winperf itself never runs elevated: `sensor_setup` relaunches
//! winperf.exe through UAC with `--sensors-setup` / `--sensors-remove`, and that
//! short-lived elevated process
//!  1. checks the bundled files against the SHA-256 manifest compiled into
//!     this exe (built by sensors-helper/build.sh),
//!  2. installs the bundled, signed PawnIO driver if it is missing,
//!  3. copies the helper to %ProgramFiles%\Fob\sensors (admin-only
//!     writable, so nothing unelevated can swap the exe the task runs elevated)
//!     and re-checks the copies,
//!  4. registers the `fob-sensors` task (at logon, highest privileges,
//!     interactive token) and starts it.
//! Because the task points at the Program Files copy, moving winperf.exe does
//! not break it; a changed bundled helper shows up as `update_available`.
// Most of this is only reachable on Windows.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const TASK_NAME: &str = "fob-sensors";
pub const HELPER_EXE: &str = "fob-sensors.exe";
/// The helper's ETW session name (sensors-helper/src/NetTrace.cs).
pub const NET_SESSION: &str = "fob-net";
pub const PAWNIO_SETUP: &str = "PawnIO/PawnIO_setup.exe";
/// `sha256  relative/path` lines; empty when the helper wasn't staged at build time.
pub const MANIFEST: &str = include_str!(concat!(env!("OUT_DIR"), "/sensors-manifest.sha256"));

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    #[default]
    Missing,
    Ok,
    /// The task runs something other than the installed helper.
    Mismatch,
    /// Couldn't query (e.g. access denied).
    Unknown,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct SetupStatus {
    /// Windows build that carries a helper manifest.
    pub supported: bool,
    /// sensors\ and PawnIO\ folders are next to fob.exe.
    pub bundled: bool,
    pub pawnio_installed: bool,
    /// Helper copied to %ProgramFiles%\Fob\sensors.
    pub installed: bool,
    pub task: TaskState,
    /// The bundled helper differs from the installed copy.
    pub update_available: bool,
    /// The helper answered on the pipe within the last seconds.
    pub running: bool,
    /// Debug-only simulated status (screenshots).
    pub fake: bool,
}

/// Parse `sha256  path` lines (sha256sum format).
pub fn manifest_entries(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (hash, path) = line.split_once("  ")?;
            let hash = hash.trim().to_ascii_lowercase();
            let path = path.trim().trim_start_matches("./").to_owned();
            (hash.len() == 64
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
                && !path.is_empty()
                && !path.split('/').any(|p| p == ".." || p.is_empty()))
            .then_some((hash, path))
        })
        .collect()
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Check every manifest entry under `prefix` (e.g. "sensors/") inside `root`,
/// where `root` holds the files with `prefix` stripped.
pub fn verify(root: &Path, manifest: &str, prefix: &str) -> Result<usize, String> {
    let entries: Vec<_> = manifest_entries(manifest)
        .into_iter()
        .filter_map(|(h, p)| p.strip_prefix(prefix).map(|rest| (h, rest.to_owned())))
        .collect();
    if entries.is_empty() {
        return Err(format!("no {prefix} files in this build's manifest"));
    }
    for (hash, rel) in &entries {
        let path = root.join(rel);
        let actual = sha256_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if &actual != hash {
            return Err(format!("{} does not match this Fob build", path.display()));
        }
    }
    Ok(entries.len())
}

/// `<Command>` of a task's XML (as returned by IRegisteredTask::Xml).
pub fn task_command(xml: &str) -> Option<String> {
    let start = xml.find("<Command>")? + "<Command>".len();
    let end = start + xml[start..].find("</Command>")?;
    Some(unescape(xml[start..end].trim().trim_matches('"')))
}

pub fn task_state(query: Result<&str, bool>, expected: &Path) -> TaskState {
    match query {
        Ok(xml) => match task_command(xml) {
            Some(cmd) if cmd.eq_ignore_ascii_case(&expected.to_string_lossy()) => TaskState::Ok,
            _ => TaskState::Mismatch,
        },
        Err(true) => TaskState::Missing,
        Err(false) => TaskState::Unknown,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Task Scheduler XML: start the helper at `user`'s logon with their full
/// (elevated) token in their session, no time limit, also on battery.
pub fn task_xml(user: &str, command: &Path) -> String {
    let dir = command.parent().unwrap_or(command);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Fob</Author>
    <Description>Fob read-only sensor helper: reads temperatures, fan speeds and per-app network bytes for Fob. Never changes fan settings.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{command}"</Command>
      <WorkingDirectory>{dir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
        user = escape(user),
        command = escape(&command.to_string_lossy()),
        dir = escape(&dir.to_string_lossy()),
    )
}

/// Exit codes of the elevated `--sensors-setup` / `--sensors-remove` run.
pub fn exit_message(code: i32) -> String {
    match code {
        0 => "Done.".into(),
        10 => "The bundled sensor files don't match this Fob build. Re-extract the whole Fob folder and try again.".into(),
        11 => "The PawnIO driver installation did not complete.".into(),
        12 => "Couldn't copy the sensor helper to Program Files.".into(),
        13 => "Couldn't create the fob-sensors scheduled task.".into(),
        14 => "Couldn't remove the fob-sensors scheduled task.".into(),
        c => format!("Setup failed (code {c})."),
    }
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default()
}

/// sensors\ next to winperf.exe.
pub fn bundled_dir() -> PathBuf {
    exe_dir().join("sensors")
}

pub fn install_dir() -> PathBuf {
    let base = std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    base.join("Fob").join("sensors")
}

pub fn installed_helper() -> PathBuf {
    install_dir().join(HELPER_EXE)
}

#[cfg(debug_assertions)]
fn fake_status() -> Option<SetupStatus> {
    let mode = std::env::var("WINPERF_FAKE_SENSOR_SETUP").ok()?;
    let ready = mode == "ready";
    Some(SetupStatus {
        supported: true,
        bundled: true,
        pawnio_installed: ready,
        installed: ready,
        task: if ready {
            TaskState::Ok
        } else {
            TaskState::Missing
        },
        update_available: false,
        running: super::pipe::helper_live(),
        fake: true,
    })
}
#[cfg(not(debug_assertions))]
fn fake_status() -> Option<SetupStatus> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn status() -> SetupStatus {
    fake_status().unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
fn run_elevated(_arg: &str) -> Result<(), String> {
    Err("Sensor setup is only available on Windows.".into())
}

#[cfg(not(target_os = "windows"))]
pub fn handle_cli() -> Option<i32> {
    None
}

#[cfg(target_os = "windows")]
use win::run_elevated;
#[cfg(target_os = "windows")]
pub use win::{handle_cli, run_task, status};

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use crate::sensors::task;
    use std::process::Command;
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY,
    };
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    /// Ask Task Scheduler to start the helper (it runs elevated without a
    /// prompt because the task was registered elevated). No child process.
    pub fn run_task() {
        let _ = task::run(TASK_NAME);
    }

    pub fn pawnio_installed() -> bool {
        let mut key = HKEY::default();
        let opened = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                w!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO"),
                Some(0),
                KEY_READ | KEY_WOW64_64KEY,
                &mut key,
            )
        };
        if opened.is_ok() {
            unsafe {
                let _ = RegCloseKey(key);
            }
            true
        } else {
            false
        }
    }

    fn query_task() -> Result<String, bool> {
        match task::query(TASK_NAME) {
            task::Query::Xml(xml) => Ok(xml),
            task::Query::Missing => Err(true),
            task::Query::Unknown => Err(false),
        }
    }

    fn same_file(a: &Path, b: &Path) -> bool {
        match (std::fs::read(a), std::fs::read(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    pub fn status() -> SetupStatus {
        if let Some(fake) = fake_status() {
            return fake;
        }
        let bundled_helper = bundled_dir().join(HELPER_EXE);
        let installed = installed_helper();
        SetupStatus {
            supported: !MANIFEST.trim().is_empty(),
            bundled: bundled_helper.exists() && exe_dir().join(PAWNIO_SETUP).exists(),
            pawnio_installed: pawnio_installed(),
            installed: installed.exists(),
            task: task_state(query_task().as_deref().map_err(|e| *e), &installed),
            update_available: installed.exists()
                && bundled_helper.exists()
                && !same_file(&bundled_helper, &installed),
            running: crate::sensors::pipe::helper_live(),
            fake: false,
        }
    }

    /// Relaunch this exe elevated (one UAC prompt) and wait for it.
    pub fn run_elevated(arg: &str) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let file = HSTRING::from(exe.as_os_str());
        let params = HSTRING::from(arg);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
            return Err(if e.code() == ERROR_CANCELLED.to_hresult() {
                "The Windows permission prompt was cancelled. Nothing was changed.".into()
            } else {
                format!("Couldn't start the elevated setup: {e}")
            });
        }
        let mut code = 1u32;
        unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            let _ = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
        }
        match code as i32 {
            0 => Ok(()),
            c => Err(exit_message(c)),
        }
    }

    /// `winperf.exe --sensors-setup|--sensors-remove`: the elevated half.
    pub fn handle_cli() -> Option<i32> {
        let arg = std::env::args().nth(1)?;
        match arg.as_str() {
            "--sensors-setup" => Some(setup().err().unwrap_or(0)),
            "--sensors-remove" => Some(remove().err().unwrap_or(0)),
            _ => None,
        }
    }

    fn stop_task() {
        let _ = task::stop(TASK_NAME);
        std::thread::sleep(std::time::Duration::from_millis(1500));
        // A killed helper can't stop its ETW session; we're elevated here.
        task::stop_trace_session(NET_SESSION);
    }

    fn setup() -> Result<(), i32> {
        let source = exe_dir();
        verify(&source.join("sensors"), MANIFEST, "sensors/").map_err(|_| 10)?;
        let target = install_dir();
        let base = target
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| target.clone());

        if !pawnio_installed() {
            // Copy into the admin-only folder first, check it there, then run.
            std::fs::create_dir_all(&base).map_err(|_| 12)?;
            let setup = base.join("PawnIO_setup.exe");
            std::fs::copy(source.join(PAWNIO_SETUP), &setup).map_err(|_| 12)?;
            let pinned = manifest_entries(MANIFEST)
                .into_iter()
                .find(|(_, p)| p == PAWNIO_SETUP)
                .map(|(h, _)| h);
            if pinned.is_none() || sha256_file(&setup).ok() != pinned {
                let _ = std::fs::remove_file(&setup);
                return Err(10);
            }
            // PawnIO's own installer window (not silent): the user sees what
            // is installed, and nothing runs hidden.
            let ran = Command::new(&setup).arg("-install").status();
            let _ = std::fs::remove_file(&setup);
            if ran.is_err() || !pawnio_installed() {
                return Err(11);
            }
        }

        stop_task();
        let mut copied = false;
        for _ in 0..3 {
            let _ = std::fs::remove_dir_all(&target);
            let ok = std::fs::create_dir_all(&target).is_ok()
                && manifest_entries(MANIFEST)
                    .iter()
                    .filter_map(|(_, p)| p.strip_prefix("sensors/"))
                    .all(|rel| {
                        std::fs::copy(source.join("sensors").join(rel), target.join(rel)).is_ok()
                    });
            if ok && verify(&target, MANIFEST, "sensors/").is_ok() {
                copied = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        if !copied {
            return Err(12);
        }

        let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
            (Ok(d), Ok(u)) if !d.is_empty() => format!(r"{d}\{u}"),
            (_, Ok(u)) => u,
            _ => return Err(13),
        };
        let xml = task_xml(&user, &target.join(HELPER_EXE));
        task::register(TASK_NAME, &xml).map_err(|_| 13)?;
        run_task();
        Ok(())
    }

    fn remove() -> Result<(), i32> {
        stop_task();
        let deleted = task::delete(TASK_NAME);
        let target = install_dir();
        for _ in 0..3 {
            if !target.exists() || std::fs::remove_dir_all(&target).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        if let Some(base) = target.parent() {
            let _ = std::fs::remove_dir(base); // only if empty
        }
        let gone = query_task() == Err(true);
        if deleted.is_ok() || gone {
            Ok(())
        } else {
            Err(14)
        }
    }
}

#[tauri::command]
pub async fn sensor_setup_status() -> SetupStatus {
    tauri::async_runtime::spawn_blocking(status)
        .await
        .unwrap_or_default()
}

/// Enable (`true`) or disable the helper. The UI explains what happens and
/// asks first; this shows the Windows UAC prompt.
#[tauri::command]
pub async fn sensor_setup(enable: bool) -> Result<SetupStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if fake_status().is_some() {
            return Err("Debug build: sensor setup is simulated and does nothing.".into());
        }
        run_elevated(if enable {
            "--sensors-setup"
        } else {
            "--sensors-remove"
        })?;
        Ok(status())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_lines_are_validated() {
        let h = "a".repeat(64);
        let text = format!(
            "{h}  sensors/fob-sensors.exe\n{h}  ./PawnIO/PawnIO_setup.exe\n\
             {h}  sensors/../evil.exe\nzz  sensors/x.dll\n{h} sensors/one-space.dll\n"
        );
        let e = manifest_entries(&text);
        assert_eq!(
            e.iter().map(|(_, p)| p.as_str()).collect::<Vec<_>>(),
            ["sensors/fob-sensors.exe", "PawnIO/PawnIO_setup.exe"]
        );
    }

    #[test]
    fn verify_checks_hashes_under_a_prefix() {
        let dir = std::env::temp_dir().join(format!("winperf-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.dll"), b"hello").unwrap();
        let good = format!(
            "{}  sensors/a.dll\n{}  PawnIO/PawnIO_setup.exe\n",
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
            "0".repeat(64)
        );
        assert_eq!(verify(&dir, &good, "sensors/"), Ok(1));
        std::fs::write(dir.join("a.dll"), b"hellO").unwrap();
        assert!(verify(&dir, &good, "sensors/").is_err());
        assert!(verify(&dir, "", "sensors/").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn task_xml_round_trips_the_command_and_escapes() {
        let exe = Path::new(r"C:\Program Files\winperf & co\sensors\winperf-sensors.exe");
        let xml = task_xml(r"DESKTOP-TEST\User", exe);
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("winperf &amp; co"));
        assert_eq!(task_command(&xml).as_deref(), Some(exe.to_str().unwrap()));
        assert_eq!(task_state(Ok(&xml), exe), TaskState::Ok);
        let moved = Path::new(r"D:\tools\winperf\sensors\winperf-sensors.exe");
        assert_eq!(task_state(Ok(&xml), moved), TaskState::Mismatch);
        assert_eq!(task_state(Err(true), exe), TaskState::Missing);
        assert_eq!(task_state(Err(false), exe), TaskState::Unknown);
    }

    #[test]
    fn exit_codes_have_messages() {
        assert!(exit_message(10).contains("don't match"));
        assert!(exit_message(99).contains("99"));
    }
}
