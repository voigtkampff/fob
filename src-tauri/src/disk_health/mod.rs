//! Physical disk health for the Disk detail: NVMe SMART / Health log fields
//! through IOCTL_STORAGE_QUERY_PROPERTY and the Windows health status from
//! MSFT_PhysicalDisk. Read only, no elevation, read on demand and cached.

mod parse;
#[cfg(windows)]
mod windows;

use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub use parse::DeviceInfo;

/// STORAGE_BUS_TYPE, which MSFT_PhysicalDisk.BusType shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bus {
    Nvme,
    Sata,
    Ata,
    Atapi,
    Usb,
    Raid,
    Scsi,
    Sas,
    Sd,
    Mmc,
    Virtual,
    Spaces,
    Other,
    Unknown,
}

impl Bus {
    pub fn from_storage_bus(v: u32) -> Self {
        match v {
            1 => Bus::Scsi,
            2 => Bus::Atapi,
            3 => Bus::Ata,
            7 => Bus::Usb,
            8 => Bus::Raid,
            10 => Bus::Sas,
            11 => Bus::Sata,
            12 => Bus::Sd,
            13 => Bus::Mmc,
            14 | 15 => Bus::Virtual,
            16 => Bus::Spaces,
            17 => Bus::Nvme,
            0 => Bus::Unknown,
            _ => Bus::Other,
        }
    }
    /// Buses whose disks cannot answer an NVMe log query, so asking is
    /// skipped. Everything else is asked, including USB and RAID, because
    /// some bridges and controllers pass the query through.
    pub fn skips_nvme_query(self) -> bool {
        matches!(
            self,
            Bus::Sata | Bus::Ata | Bus::Atapi | Bus::Sd | Bus::Mmc | Bus::Virtual | Bus::Spaces
        )
    }
}

/// Critical Warning, byte 0 of the health log. Each flag is the drive's own
/// report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CriticalWarning {
    pub spare_low: bool,
    pub temperature: bool,
    pub reliability: bool,
    pub read_only: bool,
    pub backup_failed: bool,
    pub pmr_read_only: bool,
    pub raw: u8,
}

impl CriticalWarning {
    pub fn from_bits(b: u8) -> Self {
        let bit = |n: u8| b & (1 << n) != 0;
        CriticalWarning {
            spare_low: bit(0),
            temperature: bit(1),
            reliability: bit(2),
            read_only: bit(3),
            backup_failed: bit(4),
            pmr_read_only: bit(5),
            raw: b,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NvmeHealth {
    pub critical_warning: CriticalWarning,
    pub temperature_c: Option<i32>,
    pub available_spare: Option<u8>,
    pub spare_threshold: Option<u8>,
    pub percent_used: u8,
    pub data_read_bytes: f64,
    pub data_written_bytes: f64,
    pub power_cycles: f64,
    pub power_on_hours: f64,
    pub unsafe_shutdowns: f64,
    pub media_errors: f64,
    pub error_log_entries: f64,
    pub warning_temp_minutes: u32,
    pub critical_temp_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NvmeState {
    Ok {
        log: NvmeHealth,
    },
    /// The bus cannot carry an NVMe query (SATA, ATA, virtual disks).
    NotAsked,
    /// The device, bridge or controller does not answer the query.
    Unsupported,
    Denied,
    /// The reply was too short or its header did not check out.
    BadReply,
    /// Any other Windows error; often controller specific.
    Error {
        code: u32,
    },
}

/// MSFT_PhysicalDisk.HealthStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Healthy,
    Warning,
    Unhealthy,
    Unknown,
}

impl Health {
    pub fn from_wmi(v: Option<u32>) -> Self {
        match v {
            Some(0) => Health::Healthy,
            Some(1) => Health::Warning,
            Some(2) => Health::Unhealthy,
            _ => Health::Unknown,
        }
    }
}

/// MSFT_PhysicalDisk.OperationalStatus values the UI names; others keep
/// their code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Operational {
    Ok,
    Degraded,
    Stressed,
    PredictiveFailure,
    Error,
    NonRecoverable,
    NoContact,
    LostCommunication,
    Other { code: u32 },
}

impl Operational {
    pub fn from_wmi(v: u32) -> Self {
        match v {
            2 => Operational::Ok,
            3 => Operational::Degraded,
            4 => Operational::Stressed,
            5 => Operational::PredictiveFailure,
            6 => Operational::Error,
            7 => Operational::NonRecoverable,
            12 => Operational::NoContact,
            13 => Operational::LostCommunication,
            code => Operational::Other { code },
        }
    }
}

/// One MSFT_PhysicalDisk row: only these columns are selected (no serial).
#[derive(Debug, Clone, PartialEq)]
pub struct WmiDisk {
    pub device_id: String,
    pub health: Option<u32>,
    pub operational: Vec<u32>,
    pub bus: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WindowsStatus {
    Ok {
        health: Health,
        operational: Vec<Operational>,
    },
    /// No MSFT_PhysicalDisk row matched this disk number and bus.
    NotReported,
    /// The WMI query itself failed (code is the HRESULT).
    Failed { code: i32 },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalDisk {
    /// \\.\PhysicalDriveN; None when Windows gave the disk no number.
    pub number: Option<u32>,
    pub model: Option<String>,
    pub bus: Bus,
    pub removable: bool,
    /// Mount points of volumes on this disk ("C:\"); empty for disks
    /// without drive letters.
    pub volumes: Vec<String>,
    pub windows: WindowsStatus,
    pub nvme: NvmeState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Report {
    Ok {
        checked_at_ms: u64,
        disks: Vec<PhysicalDisk>,
    },
    /// Not Windows.
    #[cfg_attr(windows, allow(dead_code))]
    Unavailable,
    /// The scan took longer than the wait; the next request picks it up.
    Pending,
}

/// What a probe of one disk found, before WMI and volumes are joined.
#[derive(Debug, Clone, PartialEq)]
pub struct RawDisk {
    pub number: Option<u32>,
    pub info: Option<DeviceInfo>,
    pub nvme: NvmeState,
}

/// A volume and the disk numbers its extents live on.
#[derive(Debug, Clone, PartialEq)]
pub struct RawVolume {
    pub mounts: Vec<String>,
    pub disks: Vec<u32>,
}

/// Joins probes, volumes and WMI rows. A WMI row is used only when its
/// DeviceId is this disk's number and, where both report one, the bus
/// matches; ambiguous rows are dropped rather than guessed.
pub fn assemble(
    raw: Vec<RawDisk>,
    volumes: &[RawVolume],
    wmi: &Result<Vec<WmiDisk>, i32>,
    checked_at_ms: u64,
) -> Report {
    let mut mounts: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for v in volumes {
        for &d in &v.disks {
            let list = mounts.entry(d).or_default();
            for m in &v.mounts {
                if !list.contains(m) {
                    list.push(m.clone());
                }
            }
        }
    }
    let mut disks: Vec<PhysicalDisk> = raw
        .into_iter()
        .map(|r| {
            let info = r.info.unwrap_or(DeviceInfo {
                model: None,
                bus: Bus::Unknown,
                removable: false,
            });
            let windows = match wmi {
                Err(code) => WindowsStatus::Failed { code: *code },
                Ok(rows) => {
                    let matches: Vec<&WmiDisk> = rows
                        .iter()
                        .filter(|w| {
                            r.number.is_some()
                                && w.device_id.trim().parse::<u32>().ok() == r.number
                                && (info.bus == Bus::Unknown
                                    || w.bus.is_none_or(|b| Bus::from_storage_bus(b) == info.bus))
                        })
                        .collect();
                    match matches.as_slice() {
                        [w] => WindowsStatus::Ok {
                            health: Health::from_wmi(w.health),
                            operational: w
                                .operational
                                .iter()
                                .map(|&c| Operational::from_wmi(c))
                                .collect(),
                        },
                        _ => WindowsStatus::NotReported,
                    }
                }
            };
            let mut volumes = r
                .number
                .and_then(|n| mounts.get(&n).cloned())
                .unwrap_or_default();
            volumes.sort();
            PhysicalDisk {
                number: r.number,
                model: info.model,
                bus: info.bus,
                removable: info.removable,
                volumes,
                windows,
                nvme: r.nvme,
            }
        })
        .collect();
    disks.sort_by_key(|d| (d.number.is_none(), d.number));
    Report::Ok {
        checked_at_ms,
        disks,
    }
}

pub const TTL: Duration = Duration::from_secs(5 * 60);
pub const WAIT: Duration = Duration::from_secs(20);

#[derive(Default)]
struct Inner {
    last: Option<(Instant, Report)>,
    scanning: bool,
}

/// One scan at a time, results reused for `TTL`. Nothing runs unless the
/// Disk detail asks.
#[derive(Default, Clone)]
pub struct DiskHealthState(Arc<(Mutex<Inner>, Condvar)>);

impl DiskHealthState {
    pub fn get<F>(&self, ttl: Duration, wait: Duration, scan: F) -> Report
    where
        F: FnOnce() -> Report + Send + 'static,
    {
        let (lock, done) = &*self.0;
        let mut inner = lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, report)) = &inner.last {
            if at.elapsed() < ttl {
                return report.clone();
            }
        }
        if !inner.scanning {
            inner.scanning = true;
            let shared = self.0.clone();
            std::thread::spawn(move || {
                let report = scan();
                let (lock, done) = &*shared;
                let mut inner = lock.lock().unwrap_or_else(|e| e.into_inner());
                inner.last = Some((Instant::now(), report));
                inner.scanning = false;
                done.notify_all();
            });
        }
        let (inner, _) = done
            .wait_timeout_while(inner, wait, |i| i.scanning)
            .unwrap_or_else(|e| e.into_inner());
        match (&inner.last, inner.scanning) {
            (Some((_, report)), false) => report.clone(),
            _ => Report::Pending,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn scan() -> Report {
    #[cfg(windows)]
    {
        let (raw, volumes, wmi) = windows::probe();
        assemble(raw, &volumes, &wmi, now_ms())
    }
    #[cfg(not(windows))]
    {
        let _ = now_ms;
        Report::Unavailable
    }
}

#[cfg(test)]
mod tests;
