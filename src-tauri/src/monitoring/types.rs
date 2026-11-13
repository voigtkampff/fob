use serde::Serialize;
use std::fmt::Debug;
use sysinfo::{DiskUsage, ProcessStatus};

/// Internal representation of process data collected from the system
/// This struct is used internally and not exposed directly to the frontend
#[derive(Clone, Debug)]
pub(crate) struct ProcessData {
    /// Process ID
    pub pid: u32,
    /// Name of the process
    pub name: String,
    /// Complete command line arguments; `None` when the cache already holds them
    pub cmd: Option<Vec<String>>,
    /// User ID that owns the process
    pub user_id: Option<String>,
    /// CPU usage as percentage of the whole machine (0-100); None without a valid reading.
    pub cpu_usage: Option<f32>,
    /// Physical memory usage in bytes
    pub memory: u64,
    /// Current process status
    pub status: ProcessStatus,
    /// Parent process ID
    pub ppid: Option<u32>,
    /// Environment variables; `None` when the cache already holds them
    pub environ: Option<Vec<String>>,
    /// Root directory of the process
    pub root: String,
    /// Virtual memory usage in bytes
    pub virtual_memory: u64,
    /// Process start time (Unix timestamp)
    pub start_time: u64,
    /// Process running time in seconds
    pub run_time: u64,
    /// Cumulative I/O counters (the totals are used, not sysinfo's deltas)
    pub disk_usage: DiskUsage,
    /// Session ID of the process
    pub session_id: Option<u32>,
}

/// Static information about a process that doesn't change frequently
/// Used for caching purposes to avoid frequent updates of stable data
#[derive(Clone, Debug)]
pub struct ProcessStaticInfo {
    /// Process name
    pub name: String,
    /// Full command string
    pub command: String,
    /// Username of the process owner
    pub user: String,
    /// Environment variables, shared by every frame of this process
    pub environ: std::sync::Arc<[String]>,
}

/// Process information exposed to the frontend via Tauri
/// Contains formatted and filtered process data for UI consumption
#[derive(Clone, Serialize, Debug)]
pub struct ProcessInfo {
    /// Busiest GPU engine this process used over the last interval, percent.
    /// Windows: PDH GPU Engine counters, 0 when the process used no engine.
    /// Linux: DRM fdinfo. None: no reading (first sample, unsupported).
    pub gpu_usage: Option<f32>,
    /// Windows: dedicated GPU memory in bytes (PDH GPU Process Memory).
    /// None when the counter is unavailable and on other platforms.
    pub gpu_memory: Option<u64>,
    /// Process ID
    pub pid: u32,
    /// Parent process ID
    pub ppid: u32,
    /// Process name
    pub name: String,
    /// CPU usage as percentage of the whole machine (0-100); None without a valid reading.
    pub cpu_usage: Option<f32>,
    /// Physical memory usage in bytes
    pub memory_usage: u64,
    /// Process status as string
    pub status: String,
    /// Username of the process owner
    pub user: String,
    /// Full command string
    pub command: String,
    /// Number of threads (if available)
    pub threads: Option<u32>,
    /// Environment variables
    pub environ: std::sync::Arc<[String]>,
    /// Root directory of the process
    pub root: String,
    /// Virtual memory usage in bytes
    pub virtual_memory: u64,
    /// Process start time (Unix timestamp)
    pub start_time: u64,
    /// Process running time in seconds
    pub run_time: u64,
    /// Disk read and write speed in bytes per second over the last capture
    /// interval. Both are None together: first reading, unreadable counters,
    /// or a counter gap.
    pub disk_read: Option<u64>,
    pub disk_write: Option<u64>,
    /// Session ID of the process
    pub session_id: Option<u32>,
    /// Local bound ports (TCP LISTEN + UDP). Empty when port collection is skipped.
    pub ports: Vec<u16>,
}

/// System-wide statistics exposed to the frontend
/// Provides overall system resource usage and performance metrics
#[derive(Clone, Serialize, Debug)]
pub struct SystemStats {
    pub details: ResourceDetails,
    /// CPU usage per core as percentage (0-100)
    pub cpu_usage: Vec<f32>,
    /// Fob: performance / efficiency groups of `cpu_usage`, read once.
    pub cpu_topology: super::CpuTopology,
    /// Total physical memory in bytes
    pub memory_total: u64,
    /// Used physical memory in bytes
    pub memory_used: u64,
    /// Free physical memory in bytes
    pub memory_free: u64,
    /// Cached memory in bytes
    pub memory_cached: u64,
    /// System uptime in seconds
    pub uptime: u64,
    /// Load averages for 1, 5, and 15 minutes
    pub load_avg: [f64; 3],
    /// Total bytes received over network
    pub network_rx_bytes: Option<u64>,
    /// Total bytes transmitted over network
    pub network_tx_bytes: Option<u64>,
    /// Total disk space in bytes
    pub disk_total_bytes: u64,
    /// Used disk space in bytes
    pub disk_used_bytes: u64,
    /// Free disk space in bytes
    pub disk_free_bytes: u64,
    /// Fob: bytes/s read from and written to the counted drives (None until
    /// two readings exist or when no drive reports I/O counters).
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
    /// Fob: bytes/s read / written (None: not reported for this drive).
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
