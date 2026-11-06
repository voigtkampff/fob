//! Process monitoring functionality
//!
//! This module handles monitoring and managing system processes, including
//! collecting process information and managing process lifecycle.

use super::{ProcessData, ProcessInfo, ProcessStaticInfo};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};
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
}

impl ProcessMonitor {
    /// Creates a new process monitor instance
    pub fn new() -> Self {
        Self {
            process_cache: HashMap::new(),
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
    pub fn collect_processes(&mut self, sys: &sysinfo::System) -> Result<Vec<ProcessInfo>, String> {
        let current_time = Self::get_current_time()?;
        let processes_data = self.collect_process_data(sys, current_time);
        Ok(self.build_process_info(processes_data))
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
    fn build_process_info(&mut self, processes: Vec<ProcessData>) -> Vec<ProcessInfo> {
        let infos: Vec<ProcessInfo> = processes
            .into_iter()
            .map(|data| {
                let cached = self
                    .process_cache
                    .entry((data.pid, data.start_time))
                    .or_insert_with(|| ProcessStaticInfo {
                        name: data.name.clone(),
                        command: String::new(),
                        user: "-".to_string(),
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
                if cached.user == "-" {
                    if let Some(user) = data.user_id {
                        cached.user = user;
                    }
                }

                ProcessInfo {
                    gpu_usage: None,
                    ports: vec![],
                    pid: data.pid,
                    ppid: data.ppid.unwrap_or(0),
                    name: cached.name.clone(),
                    cpu_usage: data.cpu_usage,
                    memory_usage: data.memory,
                    status: Self::format_status(data.status),
                    user: cached.user.clone(),
                    command: cached.command.clone(),
                    threads: None,
                    environ: cached.environ.clone(),
                    root: data.root,
                    virtual_memory: data.virtual_memory,
                    start_time: data.start_time,
                    run_time: data.run_time,
                    disk_usage: (data.disk_usage.read_bytes, data.disk_usage.written_bytes),
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
    use sysinfo::System;

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
        monitor.build_process_info(vec![data(7, 100, "old.exe", Some(&["old.exe", "-x"]))]);
        let now = monitor.build_process_info(vec![data(7, 200, "new.exe", Some(&["new.exe"]))]);
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
            monitor.build_process_info(live);
        }
        assert_eq!(monitor.process_cache.len(), 10);
    }

    #[test]
    fn later_frames_keep_command_and_environment_without_copying() {
        let mut monitor = ProcessMonitor::new();
        let first = monitor.build_process_info(vec![data(5, 1, "a", Some(&["a", "--b"]))]);
        // Cached: the collector skips cmd/environ (None).
        let second = monitor.build_process_info(vec![data(5, 1, "a", None)]);
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
        monitor.build_process_info(vec![data(5, 1, "a", Some(&[]))]);
        let later = monitor.build_process_info(vec![data(5, 1, "a", Some(&["a", "-v"]))]);
        assert_eq!(later[0].command, "a -v");
    }

    /// Tests creation of a new process monitor
    #[test]
    fn cpu_is_a_share_of_the_whole_machine() {
        // node at 115.8 % of one core on a 24-thread example hybrid CPU
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

        let result = monitor.collect_processes(&sys);
        assert!(result.is_ok());
    }
}
