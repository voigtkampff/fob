//! Application state management
//!
//! This module handles the global application state, including system monitoring
//! and process tracking capabilities.

use crate::monitoring::{ProcessMonitor, SystemMonitor};
use std::sync::Mutex;
use sysinfo::{Disks, Networks, RefreshKind, System};

/// Global application state
///
/// Maintains thread-safe access to system information and monitoring components
#[derive(Debug)]
pub struct AppState {
    /// Icons are resolved separately from the sampling loop.
    pub icons: Mutex<crate::native_icons::IconCache>,
    pub hub: Mutex<crate::sampling::Hub>,
    pub gpu: Mutex<crate::gpu::GpuMonitor>,
    /// winperf: temperatures and fans (LibreHardwareMonitor, NVML).
    pub sensors: Mutex<crate::sensors::SensorHub>,
    /// System information handler
    pub sys: Mutex<System>,
    /// Networks information handler
    pub networks: Mutex<Networks>,
    /// Networks information handler
    pub disks: Mutex<Disks>,
    /// Process monitoring component
    pub process_monitor: Mutex<ProcessMonitor>,
    /// System statistics monitoring component
    pub system_monitor: Mutex<SystemMonitor>,
}

impl AppState {
    pub fn refresh_kind() -> RefreshKind {
        let kind = RefreshKind::everything();
        #[cfg(target_os = "windows")]
        let kind = kind.with_processes(sysinfo::ProcessRefreshKind::everything().without_cpu());
        kind
    }

    /// Creates a new instance of the application state
    ///
    /// Initializes system monitoring and process tracking components
    ///
    /// # Returns
    ///
    /// A new `AppState` instance with initialized monitors
    pub fn new() -> Self {
        let sys = System::new_with_specifics(Self::refresh_kind());
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        Self {
            icons: Mutex::new(crate::native_icons::IconCache::default()),
            hub: Mutex::new(crate::sampling::Hub::default()),
            gpu: Mutex::new(crate::gpu::GpuMonitor::default()),
            sensors: Mutex::new(crate::sensors::SensorHub::default()),
            process_monitor: Mutex::new(ProcessMonitor::new()),
            system_monitor: Mutex::new(SystemMonitor::new(&networks)),
            sys: Mutex::new(sys),
            disks: Mutex::new(disks),
            networks: Mutex::new(networks),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_policy_changes_only_windows_process_cpu() {
        let kind = AppState::refresh_kind();
        let processes = kind.processes().unwrap();
        assert_eq!(processes.cpu(), !cfg!(target_os = "windows"));
        assert_eq!(
            processes.with_cpu(),
            sysinfo::ProcessRefreshKind::everything()
        );
        assert_eq!(
            kind.with_processes(sysinfo::ProcessRefreshKind::everything()),
            RefreshKind::everything()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_refresh_keeps_metadata_and_independent_cpu_samples() {
        use std::time::Duration;
        let mut sys = System::new_with_specifics(AppState::refresh_kind());
        let pid = sysinfo::Pid::from_u32(std::process::id());
        let mut monitor = ProcessMonitor::new();
        for sample in 0..2 {
            assert!(!sys.cpus().is_empty());
            assert!(sys.total_memory() > 0);
            let process = sys.process(pid).unwrap();
            assert!(!process.name().is_empty());
            assert!(!process.cmd().is_empty());
            assert!(process.memory() > 0);
            assert!(process.start_time() > 0);
            assert_eq!(process.accumulated_cpu_time(), 0);
            assert_eq!(process.cpu_usage(), 0.0);
            let processes = monitor
                .collect_processes(&sys, Duration::from_secs(8))
                .unwrap();
            let current = processes.iter().find(|p| p.pid == pid.as_u32()).unwrap();
            if sample == 0 {
                assert_eq!(current.cpu_usage, None);
                std::thread::sleep(Duration::from_millis(30));
                sys.refresh_specifics(AppState::refresh_kind());
            } else {
                let cpu = current.cpu_usage.unwrap();
                assert!(cpu.is_finite() && (0.0..=100.0).contains(&cpu));
            }
        }
    }
}
