use crate::monitoring::ProcessMonitor;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_processes(
    app: tauri::AppHandle,
    include_gpu: Option<bool>,
    include_ports: Option<bool>,
    history_cursor: Option<crate::sampling::HistoryCursor>,
) -> Result<crate::sampling::ProcessSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let state = app.state::<AppState>();
        let mut hub = state.hub.lock().map_err(|e| e.to_string())?;
        hub.process_gpu = include_gpu.unwrap_or(false);
        let mut snapshot = hub.read(&state)?;
        drop(hub);
        if include_ports.unwrap_or(false) {
            if snapshot.paused {
                snapshot.port_error = Some("Resume updates to search local ports.".into());
            } else {
                match crate::monitoring::collect_ports_by_pid() {
                    Ok(ports) => {
                        let frame = std::sync::Arc::make_mut(&mut snapshot.frame);
                        for process in &mut frame.processes {
                            process.ports = ports.get(&process.pid).cloned().unwrap_or_default();
                        }
                    }
                    Err(error) => snapshot.port_error = Some(error),
                }
            }
        }
        Ok(snapshot.for_main(history_cursor))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Attempts to kill a process with the specified PID
///
/// # Arguments
///
/// * `pid` - Process ID to kill
/// * `state` - The application state
///
/// # Returns
///
/// * `true` if the process was successfully killed
/// * `false` if the process couldn't be killed or wasn't found
///
/// # Errors
///
/// Returns an error string if failed to acquire lock on system state
#[tauri::command]
pub async fn kill_process(pid: u32, state: State<'_, AppState>) -> Result<bool, String> {
    let sys = state.sys.lock().map_err(|e| e.to_string())?;
    Ok(ProcessMonitor::kill_process(&sys, pid))
}

/// Resolve only requested, currently observed process identities. File paths never
/// come from the frontend; release the sampling lock before doing icon I/O.
#[tauri::command]
pub async fn get_process_icons(
    requests: Vec<crate::native_icons::IconRequest>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<crate::native_icons::IconReply>, String> {
    use crate::native_icons::{IconReply, MAX_BATCH};
    if requests.len() > MAX_BATCH {
        return Err("Too many icon requests".into());
    }
    let targets = {
        let sys = state.sys.lock().map_err(|e| e.to_string())?;
        requests
            .into_iter()
            .map(|request| {
                let path = crate::native_icons::process_target(&sys, &request);
                (request, path)
            })
            .collect::<Vec<_>>()
    };
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let state = app.state::<AppState>();
        let mut icons = state.icons.lock().map_err(|e| e.to_string())?;
        Ok(targets
            .into_iter()
            .map(|(request, path)| IconReply {
                pid: request.pid,
                start_time: request.start_time,
                icon: path.and_then(|p| icons.resolve(&p)),
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One listed process's executable, for the row menu's Open file location.
/// Paths never come from the frontend; the start time pins which process
/// answers.
#[tauri::command]
pub async fn process_location(
    pid: u32,
    start_time: u64,
    state: State<'_, AppState>,
) -> Result<crate::native_icons::Executable, String> {
    location_of(&state, pid, start_time)
}

fn location_of(
    state: &AppState,
    pid: u32,
    start_time: u64,
) -> Result<crate::native_icons::Executable, String> {
    let sys = state.sys.lock().map_err(|e| e.to_string())?;
    Ok(crate::native_icons::process_executable(
        &sys, pid, start_time,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_icons::Executable;

    #[test]
    fn location_says_gone_instead_of_failing_for_an_unknown_pid() {
        let state = AppState::new();
        assert_eq!(
            location_of(&state, u32::MAX - 1, 0).unwrap(),
            Executable::Gone
        );
        assert_eq!(
            location_of(&state, u32::MAX, u64::MAX).unwrap(),
            Executable::Gone
        );
        // pid 0 is Windows' idle process: listed without an image name, so it is
        // reported as unusable and never as a path the menu could reveal.
        assert!(matches!(
            location_of(&state, 0, 0).unwrap(),
            Executable::Gone | Executable::Unreadable
        ));
    }

    #[test]
    fn location_pins_the_start_time_of_the_pid() {
        let state = AppState::new();
        let pid = std::process::id();
        let start_time = {
            let mut sys = state.sys.lock().unwrap();
            sys.refresh_specifics(AppState::refresh_kind());
            sys.process(sysinfo::Pid::from_u32(pid))
                .expect("own process is listed")
                .start_time()
        };
        match location_of(&state, pid, start_time).unwrap() {
            Executable::Found { path } => {
                assert!(std::path::Path::new(&path).is_absolute(), "{path}");
            }
            other => panic!("own executable should be found, got {other:?}"),
        }
        // The same pid with another start time is another process: a reused
        // pid must not reveal the wrong file.
        assert_eq!(
            location_of(&state, pid, start_time.saturating_add(1)).unwrap(),
            Executable::Gone
        );
    }
}

#[tauri::command]
pub async fn disk_health(
    state: State<'_, crate::disk_health::DiskHealthState>,
) -> Result<crate::disk_health::Report, String> {
    use crate::disk_health::{scan, TTL, WAIT};
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.get(TTL, WAIT, scan))
        .await
        .map_err(|e| e.to_string())
}
