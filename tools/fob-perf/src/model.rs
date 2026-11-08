//! Raw data shapes shared by the Windows collector, the analyzer and tests.
use serde::{Deserialize, Serialize};

pub const SCHEMA: u32 = 1;

/// A process is identified by pid plus creation time (FILETIME, 100 ns units).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Identity {
    pub pid: u32,
    pub create: u64,
}

/// One row of the OS process list (Toolhelp). No command line or environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Root,
    Child,
}

/// Metrics read for one owned process at one sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Obs {
    pub id: Identity,
    pub ppid: u32,
    pub name: String,
    pub role: Role,
    /// Kernel + user CPU time since creation, 100 ns units.
    pub cpu: Option<u64>,
    /// PROCESS_MEMORY_COUNTERS_EX.PrivateUsage: private committed bytes.
    pub private_bytes: Option<u64>,
    /// Working set bytes (includes shared pages; summing can double-count).
    pub working_set: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootState {
    Alive,
    Exited,
    PidReused,
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Excluded {
    pub pid: u32,
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub seq: u64,
    /// Monotonic ns since the sampler started.
    pub t_ns: u64,
    /// Wall clock FILETIME at the sample (to tell children born mid-interval).
    pub wall: u64,
    pub root: RootState,
    pub procs: Vec<Obs>,
    pub excluded: Vec<Excluded>,
    pub discovery_error: Option<String>,
}

/// Everything about a run except samples. Written once to run.json.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunMeta {
    pub schema: u32,
    pub run_id: String,
    pub tool_version: String,
    pub label: String,
    pub app_version: String,
    pub mode: String,
    pub refresh_ms: u32,
    pub helper: String,
    pub settings: Vec<String>,
    pub interval_ms: u64,
    pub settle_s: u64,
    pub steady_s: u64,
    pub logical_cores: u32,
    pub processor_groups: u32,
    pub machine: String,
    pub started_utc: String,
    pub root: Identity,
    pub root_exe_name: String,
    pub root_exe_sha256: Option<String>,
    pub root_exe_bytes: Option<u64>,
    pub root_age_at_attach_s: f64,
    pub scope: String,
}

pub const MODES: [&str; 4] = ["tray", "mini", "statusbar", "processes"];
pub const SCOPE: &str = "fob.exe root + descendants verified by parent pid and creation time; sensor/MFT helpers excluded";
pub const HELPERS: [&str; 2] = ["fob-sensors.exe", "fob-mft.exe"];
