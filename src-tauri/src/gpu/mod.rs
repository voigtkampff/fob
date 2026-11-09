//! Read-only, driver-reported GPU observations. Missing counters stay missing.
use serde::Serialize;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(all(test, target_os = "macos"))]
#[path = "linux.rs"]
mod linux_fixtures;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod nvidia;
#[cfg(target_os = "windows")]
mod windows;
// winperf: PDH GPU engine counter parsing (used on Windows, tested everywhere).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod engine_counters;
pub use engine_counters::ProcessGpu;

#[derive(Clone, Debug, Serialize)]
pub struct GpuDevice {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub utilization: Option<f64>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub memory_kind: String,
    pub temperature: Option<f64>,
    pub power_watts: Option<f64>,
    pub core_clock_mhz: Option<u64>,
    pub cores: Option<u64>,
    pub source: String,
    /// winperf: fan duty in percent (NVML), when the driver reports it.
    pub fan_percent: Option<f64>,
    /// winperf: (engine type, utilization %) on Windows PDH; empty elsewhere.
    pub engines: Vec<(String, f64)>,
}

pub fn percent(value: f64) -> Option<f64> {
    (value.is_finite() && (0.0..=100.0).contains(&value)).then_some(value)
}

#[derive(Debug, Default)]
pub struct GpuMonitor {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    nvidia: nvidia::Nvidia,
    #[cfg(target_os = "windows")]
    engines: windows::EngineCounters,
    /// Per-process GPU % and dedicated memory from the last sample (Windows
    /// PDH only). None: no reading this sample, so every process is unknown.
    pub process_usage: Option<ProcessGpu>,
}

impl GpuMonitor {
    pub fn collect(&mut self) -> Vec<GpuDevice> {
        #[cfg(target_os = "macos")]
        let mut devices = macos::collect();
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        let mut devices = {
            let mut devices = self.nvidia.collect();
            #[cfg(target_os = "linux")]
            devices.extend(linux::collect());
            #[cfg(target_os = "windows")]
            {
                let mut covered = devices
                    .iter()
                    .map(|d| d.name.trim_start_matches("NVIDIA ").to_lowercase())
                    .collect::<Vec<_>>();
                let busy = self.engines.sample().unwrap_or_default();
                self.process_usage = ProcessGpu::from_sample(&busy);
                for device in windows::collect(&busy) {
                    if device.vendor == "NVIDIA" {
                        let name = device.name.trim_start_matches("NVIDIA ").to_lowercase();
                        if let Some(index) = covered.iter().position(|n| *n == name) {
                            covered.remove(index);
                            continue;
                        }
                    }
                    devices.push(device);
                }
            }
            devices
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        let mut devices: Vec<GpuDevice> = Vec::new();
        #[cfg(debug_assertions)]
        devices.extend(fake_gpu());
        devices.sort_by(|a, b| a.id.cmp(&b.id));
        devices.dedup_by(|a, b| a.id == b.id);
        devices.truncate(8);
        devices
    }
}

/// Debug builds only (screenshots on a machine without a GPU):
/// `WINPERF_FAKE_GPU=1` adds one adapter with a slowly varying load.
#[cfg(debug_assertions)]
fn fake_gpu() -> Option<GpuDevice> {
    std::env::var_os("WINPERF_FAKE_GPU")?;
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs_f64();
    Some(GpuDevice {
        id: "fake-gpu".into(),
        name: "NVIDIA Example GPU".into(),
        vendor: "NVIDIA".into(),
        utilization: Some((35.0 + 25.0 * (t / 9.0).sin() + 10.0 * (t / 2.3).sin()).max(0.0)),
        memory_used: Some(1_700_000_000),
        memory_total: Some(24 * 1024 * 1024 * 1024),
        memory_kind: "dedicated".into(),
        temperature: Some(40.0),
        fan_percent: Some(0.0),
        power_watts: None,
        core_clock_mhz: None,
        cores: None,
        source: "fake".into(),
        engines: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_is_not_idle() {
        assert_eq!(percent(0.0), Some(0.0));
        assert_eq!(percent(100.0), Some(100.0));
        for value in [-1.0, 101.0, f64::NAN, f64::INFINITY] {
            assert_eq!(percent(value), None);
        }
    }
}
