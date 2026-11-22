use serde::Serialize;
use std::fmt::Debug;
use sysinfo::{DiskUsage, ProcessStatus};

#[derive(Clone, Debug)]
pub(crate) struct ProcessData {
    pub pid: u32,
    pub name: String,
    pub cmd: Option<Vec<String>>,
    pub user_id: Option<String>,
    pub cpu_usage: Option<f32>,
    pub memory: u64,
    pub status: ProcessStatus,
    pub ppid: Option<u32>,
    #[cfg(target_os = "windows")]
    pub environment_id: Option<u64>,
    pub root: String,
    pub virtual_memory: u64,
    pub start_time: u64,
    pub run_time: u64,
    pub disk_usage: DiskUsage,
    pub session_id: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct ProcessStaticInfo {
    pub name: String,
    pub command: String,
    pub user: String,
}

#[derive(Clone, Serialize, Debug)]
pub struct ProcessInfo {
    /// Busiest GPU engine this process used over the last interval, percent.
    /// Windows: PDH GPU Engine counters, 0 when the process used no engine.
    /// Linux: DRM fdinfo. None: no reading (first sample, unsupported).
    pub gpu_usage: Option<f32>,
    /// Windows: dedicated GPU memory in bytes (PDH GPU Process Memory).
    /// None when the counter is unavailable and on other platforms.
    pub gpu_memory: Option<u64>,
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub cpu_usage: Option<f32>,
    pub memory_usage: u64,
    pub status: String,
    pub user: String,
    pub command: String,
    pub threads: Option<u32>,
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
    pub root: String,
    pub virtual_memory: u64,
    pub start_time: u64,
    pub run_time: u64,
    pub disk_read: Option<u64>,
    pub disk_write: Option<u64>,
    pub session_id: Option<u32>,
    pub ports: Vec<u16>,
}

#[derive(Clone, Serialize, Debug)]
pub struct SystemStats {
    pub details: ResourceDetails,
    pub cpu_usage: Vec<f32>,
    /// Fob: effective CPU frequency, MHz (CPU base × PDH `% Processor
    /// Performance`); None while unavailable or on other platforms.
    pub cpu_clock: Option<f64>,
    pub cpu_topology: super::CpuTopology,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_free: u64,
    pub memory_cached: u64,
    pub uptime: u64,
    pub load_avg: [f64; 3],
    pub network_rx_bytes: Option<u64>,
    pub network_tx_bytes: Option<u64>,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_free_bytes: u64,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SystemIdentity {
    pub cpu_model: Option<String>,
    pub physical_cores: Option<usize>,
    pub architecture: String,
    pub platform: String,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VolumeDetails {
    pub name: String,
    pub mount: String,
    pub file_system: String,
    pub total: u64,
    pub available: u64,
    pub read_only: bool,
    pub read: Option<u64>,
    pub written: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InterfaceDetails {
    pub name: String,
    pub received: Option<u64>,
    pub sent: Option<u64>,
    pub received_total: u64,
    pub sent_total: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourceDetails {
    pub identity: SystemIdentity,
    pub memory_unallocated: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub volumes: Vec<VolumeDetails>,
    pub interfaces: Vec<InterfaceDetails>,
}
