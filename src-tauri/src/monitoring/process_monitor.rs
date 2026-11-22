use super::accounts::AccountNames;
use super::io_rates::{IoRates, IoTotals};
use super::{ProcessData, ProcessInfo, ProcessStaticInfo};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Debug;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::ProcessStatus;

fn os_string_vec_to_string_vec(v: &[OsString]) -> Vec<String> {
    v.iter()
        .map(|c| c.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
}

#[derive(Debug)]
pub struct ProcessMonitor {
    /// Static details of the live processes, keyed by (pid, start time) so a
    /// reused pid never shows the old process's name or command. Entries for
    /// processes that are gone are dropped every collection.
    process_cache: HashMap<(u32, u64), ProcessStaticInfo>,
    io: IoRates,
    /// Owner names by SID, looked up once per owner.
    accounts: AccountNames,
    #[cfg(target_os = "windows")]
    cpu: super::cpu_rates::CpuRates,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            process_cache: HashMap::new(),
            io: IoRates::default(),
            accounts: AccountNames::default(),
            #[cfg(target_os = "windows")]
            cpu: super::cpu_rates::CpuRates::default(),
        }
    }

    pub fn collect_processes(
        &mut self,
        sys: &sysinfo::System,
        max_window: Duration,
    ) -> Result<Vec<ProcessInfo>, String> {
        let current_time = Self::get_current_time()?;
        let processes_data = self.collect_process_data(sys, current_time, max_window);
        Ok(self.build_process_info(processes_data, Instant::now(), max_window))
    }

    pub fn kill_process(sys: &sysinfo::System, pid: u32) -> bool {
        sys.process(sysinfo::Pid::from(pid as usize))
            .map(|process| process.kill())
            .unwrap_or(false)
    }

    pub fn read_environment(
        pid: u32,
        start_time: u64,
        environment_id: Option<&str>,
    ) -> Option<Vec<String>> {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

        #[cfg(target_os = "windows")]
        let (expected, handle) = {
            use super::cpu_rates::{open_process, process_times};
            let expected = environment_id?.parse::<u64>().ok()?;
            let handle = open_process(pid)?;
            if process_times(*handle)?.created != expected {
                return None;
            }
            (expected, handle)
        };
        #[cfg(not(target_os = "windows"))]
        let _ = environment_id;

        let mut sys = System::new();
        let pid = Pid::from_u32(pid);
        let target = [pid];
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&target),
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_environ(UpdateKind::Always),
        );
        let process = sys
            .process(pid)
            .filter(|p| p.start_time() == start_time && start_time != 0)?;
        let variables = os_string_vec_to_string_vec(process.environ());

        #[cfg(target_os = "windows")]
        if super::cpu_rates::process_times(*handle)?.created != expected {
            return None;
        }
        #[cfg(not(target_os = "windows"))]
        {
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&target),
                true,
                ProcessRefreshKind::nothing().without_tasks(),
            );
            sys.process(pid).filter(|p| p.start_time() == start_time)?;
        }

        Some(variables)
    }

    fn get_current_time() -> Result<u64, String> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| format!("Failed to get system time: {}", e))
    }

    fn collect_process_data(
        &mut self,
        sys: &sysinfo::System,
        current_time: u64,
        _max_window: Duration,
    ) -> Vec<ProcessData> {
        #[cfg(not(target_os = "windows"))]
        let logical = logical_processors(sys);
        #[cfg(target_os = "windows")]
        let cpu = self.cpu.advance(
            Instant::now(),
            _max_window,
            super::cpu_rates::system(),
            sys.processes().iter().map(|(pid, p)| {
                (
                    (pid.as_u32(), p.start_time()),
                    super::cpu_rates::process(pid.as_u32()),
                )
            }),
        );
        sys.processes()
            .iter()
            .map(|(pid, process)| {
                let start_time = process.start_time();
                // Copy the command only until the cache holds a non-empty value.
                let cached = self.process_cache.get(&(pid.as_u32(), start_time));
                let need_cmd = cached.is_none_or(|c| c.command.is_empty());
                ProcessData {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().into_owned(),
                    cmd: need_cmd.then(|| os_string_vec_to_string_vec(process.cmd())),
                    user_id: process.user_id().map(|uid| uid.to_string()),
                    #[cfg(not(target_os = "windows"))]
                    cpu_usage: process
                        .cpu_usage()
                        .is_finite()
                        .then(|| normalize_cpu(process.cpu_usage(), logical)),
                    #[cfg(target_os = "windows")]
                    cpu_usage: cpu.get(&(pid.as_u32(), start_time)).copied(),
                    memory: process.memory(),
                    status: process.status(),
                    ppid: process.parent().map(|p| p.as_u32()),
                    #[cfg(target_os = "windows")]
                    environment_id: self.cpu.created((pid.as_u32(), start_time)),
                    root: process
                        .root()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    virtual_memory: process.virtual_memory(),
                    start_time,
                    run_time: if start_time > 0 {
                        current_time.saturating_sub(start_time)
                    } else {
                        0
                    },
                    disk_usage: process.disk_usage(),
                    session_id: process.session_id().map(|id| id.as_u32()),
                }
            })
            .collect()
    }

    fn build_process_info(
        &mut self,
        processes: Vec<ProcessData>,
        now: Instant,
        max_window: Duration,
    ) -> Vec<ProcessInfo> {
        let rates = self.io.advance(
            now,
            max_window,
            processes.iter().map(|data| {
                let usage = &data.disk_usage;
                (
                    (data.pid, data.start_time),
                    IoTotals::readable(usage.total_read_bytes, usage.total_written_bytes),
                )
            }),
        );
        let accounts = &mut self.accounts;
        let infos: Vec<ProcessInfo> = processes
            .into_iter()
            .map(|data| {
                let cached = self
                    .process_cache
                    .entry((data.pid, data.start_time))
                    .or_insert_with(|| ProcessStaticInfo {
                        name: data.name.clone(),
                        command: String::new(),
                        // The owner seen first is kept for the process; one
                        // whose token cannot be read shows a dash.
                        user: data.user_id.clone().unwrap_or_else(|| "-".to_string()),
                    });
                if let Some(cmd) = data.cmd.filter(|c| !c.is_empty()) {
                    if cached.command.is_empty() {
                        cached.command = cmd.join(" ");
                    }
                }

                let rate = rates.get(&(data.pid, data.start_time));
                ProcessInfo {
                    gpu_usage: None,
                    gpu_memory: None,
                    ports: vec![],
                    pid: data.pid,
                    ppid: data.ppid.unwrap_or(0),
                    name: cached.name.clone(),
                    cpu_usage: data.cpu_usage,
                    memory_usage: data.memory,
                    status: Self::format_status(data.status),
                    user: if cached.user == "-" {
                        cached.user.clone()
                    } else {
                        accounts.name(&cached.user)
                    },
                    command: cached.command.clone(),
                    threads: None,
                    #[cfg(target_os = "windows")]
                    environment_id: data.environment_id.map(|created| created.to_string()),
                    root: data.root,
                    virtual_memory: data.virtual_memory,
                    start_time: data.start_time,
                    run_time: data.run_time,
                    disk_read: rate.map(|r| r.read),
                    disk_write: rate.map(|r| r.written),
                    session_id: data.session_id,
                }
            })
            .collect();
        let live: std::collections::HashSet<(u32, u64)> =
            infos.iter().map(|p| (p.pid, p.start_time)).collect();
        self.process_cache.retain(|key, _| live.contains(key));
        infos
    }

    pub fn format_status(status: ProcessStatus) -> String {
        match status {
            ProcessStatus::Run => "Running",
            ProcessStatus::Sleep => "Sleeping",
            ProcessStatus::Idle => "Idle",
            _ => "Unknown",
        }
        .to_string()
    }
}

#[cfg(not(target_os = "windows"))]
fn logical_processors(sys: &sysinfo::System) -> usize {
    match sys.cpus().len() {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    }
}

/// sysinfo reports a process's CPU as a share of one logical processor
/// (0..100 x logical). Task Manager shows the share of the whole machine,
/// 0..100, so a process that saturates 1 of 24 threads reads 4.2 %.
#[cfg(any(not(target_os = "windows"), test))]
fn normalize_cpu(per_core: f32, logical: usize) -> f32 {
    if !per_core.is_finite() {
        return 0.0;
    }
    (per_core / logical.max(1) as f32).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::{DiskUsage, System};

    const WINDOW: Duration = Duration::from_millis(7500);

    fn build(monitor: &mut ProcessMonitor, live: Vec<ProcessData>) -> Vec<ProcessInfo> {
        monitor.build_process_info(live, Instant::now(), WINDOW)
    }

    fn with_io(mut process: ProcessData, read: u64, written: u64) -> ProcessData {
        process.disk_usage = DiskUsage {
            total_read_bytes: read,
            total_written_bytes: written,
            ..Default::default()
        };
        process
    }

    fn data(pid: u32, start_time: u64, name: &str, cmd: Option<&[&str]>) -> ProcessData {
        let strings = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        ProcessData {
            pid,
            name: name.into(),
            cmd: cmd.map(strings),
            user_id: Some("1000".into()),
            cpu_usage: Some(1.0),
            memory: 10,
            status: ProcessStatus::Run,
            ppid: None,
            #[cfg(target_os = "windows")]
            environment_id: None,
            root: String::new(),
            virtual_memory: 0,
            start_time,
            run_time: 0,
            disk_usage: Default::default(),
            session_id: None,
        }
    }

    #[test]
    fn a_reused_pid_shows_the_new_process() {
        let mut monitor = ProcessMonitor::new();
        build(
            &mut monitor,
            vec![data(7, 100, "old.exe", Some(&["old.exe", "-x"]))],
        );
        let now = build(
            &mut monitor,
            vec![data(7, 200, "new.exe", Some(&["new.exe"]))],
        );
        assert_eq!(now[0].name, "new.exe");
        assert_eq!(now[0].command, "new.exe");
    }

    #[test]
    fn exited_processes_leave_the_cache() {
        let mut monitor = ProcessMonitor::new();
        for tick in 0..50u32 {
            let live = (0..10)
                .map(|i| data(tick * 10 + i, 1, "p", Some(&["p"])))
                .collect();
            build(&mut monitor, live);
        }
        assert_eq!(monitor.process_cache.len(), 10);
    }

    #[test]
    fn later_frames_keep_the_command() {
        let mut monitor = ProcessMonitor::new();
        let first = build(&mut monitor, vec![data(5, 1, "a", Some(&["a", "--b"]))]);
        let second = build(&mut monitor, vec![data(5, 1, "a", None)]);
        assert_eq!(second[0].command, "a --b");
        assert_eq!(
            serde_json::to_value(&first[0]).unwrap(),
            serde_json::to_value(&second[0]).unwrap()
        );
    }

    #[test]
    fn a_command_read_late_still_arrives() {
        let mut monitor = ProcessMonitor::new();
        build(&mut monitor, vec![data(5, 1, "a", Some(&[]))]);
        let later = build(&mut monitor, vec![data(5, 1, "a", Some(&["a", "-v"]))]);
        assert_eq!(later[0].command, "a -v");
    }

    #[test]
    fn an_empty_reading_keeps_no_stale_process() {
        let mut monitor = ProcessMonitor::new();
        build(
            &mut monitor,
            vec![data(9, 1, "gone.exe", Some(&["gone.exe"]))],
        );
        assert!(build(&mut monitor, vec![]).is_empty());
        assert!(monitor.process_cache.is_empty());
        let back = build(&mut monitor, vec![data(9, 1, "next.exe", None)]);
        assert_eq!(back[0].name, "next.exe");
        assert_eq!(back[0].command, "");
    }

    #[test]
    fn owners_show_account_names_looked_up_once_per_sid() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn slow(sid: &str) -> Result<String, crate::monitoring::accounts::Unresolved> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(300));
            match sid {
                "S-1-5-18" => Ok("SYSTEM".into()),
                _ => Err(crate::monitoring::accounts::Unresolved::NoAccount),
            }
        }
        let mut monitor = ProcessMonitor::new();
        monitor.accounts = AccountNames::new(slow);
        let owned = |pid, sid: &str| {
            let mut p = data(pid, pid as u64, "svc", None);
            p.user_id = Some(sid.into());
            p
        };
        let gone = "S-1-5-21-1000000001-1000000002-1000000003-1999";
        let started = Instant::now();
        let first = build(&mut monitor, vec![owned(1, "S-1-5-18"), owned(2, gone)]);
        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(
            [first[0].user.as_str(), first[1].user.as_str()],
            ["S-1-5-18", gone]
        );
        let until = Instant::now() + Duration::from_secs(5);
        let mut pid = 3;
        let later = loop {
            let later = build(
                &mut monitor,
                vec![owned(pid, "S-1-5-18"), owned(pid + 100, gone)],
            );
            pid += 1;
            if (later[0].user == "SYSTEM" && CALLS.load(Ordering::SeqCst) == 2)
                || Instant::now() > until
            {
                break later;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(
            [later[0].user.as_str(), later[1].user.as_str()],
            ["SYSTEM", gone]
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn the_owner_seen_first_is_kept_and_missing_stays_a_dash() {
        let mut monitor = ProcessMonitor::new();
        let mut first = data(3, 1, "a", Some(&["a"]));
        first.user_id = None;
        assert_eq!(build(&mut monitor, vec![first])[0].user, "-");
        let later = build(&mut monitor, vec![data(3, 1, "a", None)]);
        assert_eq!(later[0].user, "-");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn environment_reads_only_the_live_process_with_the_matching_creation_time() {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

        let mut child = Command::new("ping.exe")
            .args(["-t", "127.0.0.1"])
            .env("FOB_TEST_PROCESS_ENV", "fixture")
            .creation_flags(0x08000000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let created = super::super::cpu_rates::process(pid).map(|c| c.created);
        let mut sys = System::new();
        let target = Pid::from_u32(pid);
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[target]),
            true,
            ProcessRefreshKind::nothing(),
        );
        let start_time = sys.process(target).map_or(0, |p| p.start_time());
        let environment_id = created.map(|created| created.to_string());
        let replacement_id = created.map(|created| (created ^ 1).to_string());
        let matching = ProcessMonitor::read_environment(pid, start_time, environment_id.as_deref())
            .is_some_and(|vars| vars.iter().any(|var| var == "FOB_TEST_PROCESS_ENV=fixture"));
        let replacement =
            ProcessMonitor::read_environment(pid, start_time, replacement_id.as_deref());
        drop(sys);
        let stopped = child.kill().and_then(|_| child.wait());
        assert!(stopped.is_ok());
        assert!(matching);
        assert!(replacement.is_none());
        assert!(
            ProcessMonitor::read_environment(pid, start_time, environment_id.as_deref()).is_none()
        );
    }

    #[test]
    fn process_snapshot_omits_the_environment() {
        let mut monitor = ProcessMonitor::new();
        let info = build(&mut monitor, vec![data(5, 1, "a", Some(&["a"]))]);
        let json = serde_json::to_value(&info[0]).unwrap();
        assert!(json.get("environ").is_none());
        assert_eq!(json["command"], "a");
        assert_eq!(json["user"], "1000");
    }

    #[test]
    fn disk_speed_is_unknown_until_two_readings_then_bytes_per_second() {
        let mut monitor = ProcessMonitor::new();
        let t0 = Instant::now();
        let p = |r, w| with_io(data(4, 1, "copy.exe", Some(&["copy.exe"])), r, w);
        let first = monitor.build_process_info(vec![p(50_000_000, 0)], t0, WINDOW);
        let json = serde_json::to_value(&first[0]).unwrap();
        assert!(json["disk_read"].is_null() && json["disk_write"].is_null());
        let busy = monitor.build_process_info(
            vec![p(50_000_000 + 6_000_000, 30_000)],
            t0 + Duration::from_secs(3),
            WINDOW,
        );
        let json = serde_json::to_value(&busy[0]).unwrap();
        assert_eq!(json["disk_read"], 2_000_000);
        assert_eq!(json["disk_write"], 10_000);
        let idle = monitor.build_process_info(
            vec![p(56_000_000, 30_000)],
            t0 + Duration::from_secs(6),
            WINDOW,
        );
        assert_eq!((idle[0].disk_read, idle[0].disk_write), (Some(0), Some(0)));
        assert!(json.get("disk_usage").is_none());
    }

    #[test]
    fn unreadable_io_counters_stay_unknown_not_idle() {
        let mut monitor = ProcessMonitor::new();
        let t0 = Instant::now();
        let p = || with_io(data(4, 1, "protected.exe", None), 0, 0);
        monitor.build_process_info(vec![p()], t0, WINDOW);
        let later = monitor.build_process_info(vec![p()], t0 + Duration::from_secs(3), WINDOW);
        assert_eq!((later[0].disk_read, later[0].disk_write), (None, None));
    }

    #[test]
    fn a_reused_pid_never_inherits_disk_speed() {
        let mut monitor = ProcessMonitor::new();
        let t0 = Instant::now();
        monitor.build_process_info(
            vec![with_io(data(7, 100, "old.exe", None), 1_000, 1_000)],
            t0,
            WINDOW,
        );
        let now = monitor.build_process_info(
            vec![with_io(data(7, 200, "new.exe", None), 90_000_000, 1)],
            t0 + Duration::from_secs(3),
            WINDOW,
        );
        assert_eq!(now[0].disk_read, None);
    }

    #[test]
    fn cpu_is_a_share_of_the_whole_machine() {
        assert!((normalize_cpu(115.8, 24) - 4.825).abs() < 1e-4);
        assert_eq!(normalize_cpu(2400.0, 24), 100.0);
        assert_eq!(normalize_cpu(3000.0, 24), 100.0);
        assert_eq!(normalize_cpu(50.0, 1), 50.0);
        assert_eq!(normalize_cpu(50.0, 0), 50.0);
        assert_eq!(normalize_cpu(-1.0, 8), 0.0);
        assert_eq!(normalize_cpu(f32::NAN, 8), 0.0);
    }

    #[test]
    fn test_process_monitor_creation() {
        let monitor = ProcessMonitor::new();
        assert!(monitor.process_cache.is_empty());
    }

    #[test]
    fn test_process_collection() {
        let mut monitor = ProcessMonitor::new();
        let mut sys = System::new();
        sys.refresh_all();

        let result = monitor.collect_processes(&sys, WINDOW);
        assert!(result.is_ok());
    }
}
