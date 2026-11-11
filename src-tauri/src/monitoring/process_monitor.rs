//! Process monitoring functionality
//!
//! This module handles monitoring and managing system processes, including
//! collecting process information and managing process lifecycle.

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

/// Monitors and manages system processes
#[derive(Debug)]
pub struct ProcessMonitor {
    /// Static details of the live processes, keyed by (pid, start time) so a
    /// reused pid never shows the old process's name or command. Entries for
    /// processes that are gone are dropped every collection.
    process_cache: HashMap<(u32, u64), ProcessStaticInfo>,
    /// Previous I/O counters, for disk speed per process.
    io: IoRates,
    /// Owner names by SID, looked up once per owner.
    accounts: AccountNames,
}

impl ProcessMonitor {
    /// Creates a new process monitor instance
    pub fn new() -> Self {
        Self {
            process_cache: HashMap::new(),
            io: IoRates::default(),
            accounts: AccountNames::default(),
        }
    }

    /// Collects information about all running processes
    ///
    /// # Arguments
    ///
    /// * `sys` - System information provider
    ///
    /// # Returns
    ///
    /// A vector of process information, or an error string if collection failed
    ///
    /// Call right after refreshing `sys`. Disk speeds cover the time since the
    /// previous call; a gap longer than `max_window` reports no speed.
    pub fn collect_processes(
        &mut self,
        sys: &sysinfo::System,
        max_window: Duration,
    ) -> Result<Vec<ProcessInfo>, String> {
        let current_time = Self::get_current_time()?;
        let processes_data = self.collect_process_data(sys, current_time);
        Ok(self.build_process_info(processes_data, Instant::now(), max_window))
    }

    /// Attempts to kill a process
    ///
    /// # Arguments
    ///
    /// * `sys` - System information provider
    /// * `pid` - Process ID to kill
    ///
    /// # Returns
    ///
    /// Boolean indicating whether the process was successfully killed
    pub fn kill_process(sys: &sysinfo::System, pid: u32) -> bool {
        sys.process(sysinfo::Pid::from(pid as usize))
            .map(|process| process.kill())
            .unwrap_or(false)
    }

    /// Gets the current system time in seconds since UNIX epoch
    fn get_current_time() -> Result<u64, String> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| format!("Failed to get system time: {}", e))
    }

    /// Collects raw process data from the system
    fn collect_process_data(&self, sys: &sysinfo::System, current_time: u64) -> Vec<ProcessData> {
        let logical = logical_processors(sys);
        sys.processes()
            .iter()
            .map(|(pid, process)| {
                let start_time = process.start_time();
                // sysinfo fills cmd and environ once per process; copy them
                // only until the cache holds a non-empty value.
                let cached = self.process_cache.get(&(pid.as_u32(), start_time));
                let need_cmd = cached.is_none_or(|c| c.command.is_empty());
                let need_environ = cached.is_none_or(|c| c.environ.is_empty());
                ProcessData {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().into_owned(),
                    cmd: need_cmd.then(|| os_string_vec_to_string_vec(process.cmd())),
                    user_id: process.user_id().map(|uid| uid.to_string()),
                    cpu_usage: normalize_cpu(process.cpu_usage(), logical),
                    memory: process.memory(),
                    status: process.status(),
                    ppid: process.parent().map(|p| p.as_u32()),
                    environ: need_environ.then(|| os_string_vec_to_string_vec(process.environ())),
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

    /// Builds process information from raw process data
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
                        environ: Vec::new().into(),
                    });
                if let Some(cmd) = data.cmd.filter(|c| !c.is_empty()) {
                    if cached.command.is_empty() {
                        cached.command = cmd.join(" ");
                    }
                }
                if let Some(environ) = data.environ.filter(|e| !e.is_empty()) {
                    if cached.environ.is_empty() {
                        cached.environ = environ.into();
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
                    // The account name once its background lookup is done;
                    // the SID until then.
                    user: if cached.user == "-" {
                        cached.user.clone()
                    } else {
                        accounts.name(&cached.user)
                    },
                    command: cached.command.clone(),
                    threads: None,
                    environ: cached.environ.clone(),
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

    /// Formats process status into a human-readable string
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

/// Logical processors the CPU percentages are shared across.
fn logical_processors(sys: &sysinfo::System) -> usize {
    match sys.cpus().len() {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    }
}

/// sysinfo reports a process's CPU as a share of one logical processor
/// (0..100 x logical). Task Manager shows the share of the whole machine,
/// 0..100, so a process that saturates 1 of 24 threads reads 4.2 %.
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
            cpu_usage: 1.0,
            memory: 10,
            status: ProcessStatus::Run,
            ppid: None,
            environ: cmd.map(|_| strings(&["A=1", "B=2"])),
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
    fn later_frames_keep_command_and_environment_without_copying() {
        let mut monitor = ProcessMonitor::new();
        let first = build(&mut monitor, vec![data(5, 1, "a", Some(&["a", "--b"]))]);
        // Cached: the collector skips cmd/environ (None).
        let second = build(&mut monitor, vec![data(5, 1, "a", None)]);
        assert_eq!(second[0].command, "a --b");
        assert_eq!(&*second[0].environ, &["A=1".to_string(), "B=2".to_string()]);
        assert!(std::sync::Arc::ptr_eq(
            &first[0].environ,
            &second[0].environ
        ));
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

    /// Tests creation of a new process monitor
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
        assert!(back[0].environ.is_empty());
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
        // The first refresh does not wait: it shows the SIDs.
        let started = Instant::now();
        let first = build(&mut monitor, vec![owned(1, "S-1-5-18"), owned(2, gone)]);
        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(
            [first[0].user.as_str(), first[1].user.as_str()],
            ["S-1-5-18", gone]
        );
        // Processes come and go; once the names are in, the same owners show
        // them and are not looked up again.
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

    #[test]
    fn shared_environment_serialises_like_a_list() {
        let mut monitor = ProcessMonitor::new();
        let info = build(&mut monitor, vec![data(5, 1, "a", Some(&["a"]))]);
        let json = serde_json::to_value(&info[0]).unwrap();
        assert_eq!(json["environ"], serde_json::json!(["A=1", "B=2"]));
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
        // A lifetime total is never shown as the current speed.
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
        // node at 115.8 % of one core on a 24-thread Intel hybrid CPU
        assert!((normalize_cpu(115.8, 24) - 4.825).abs() < 1e-4);
        // every thread busy is 100 %, never more
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

    /// Tests process collection functionality
    #[test]
    fn test_process_collection() {
        let mut monitor = ProcessMonitor::new();
        let mut sys = System::new();
        sys.refresh_all();

        let result = monitor.collect_processes(&sys, WINDOW);
        assert!(result.is_ok());
    }
}
