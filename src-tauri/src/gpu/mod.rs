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
                let busy = self.engines.sample().unwrap_or_default();
                self.process_usage = ProcessGpu::from_sample(&busy);
                devices = pair_nvidia(devices, windows::collect(&busy));
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

/// winperf: on Windows one NVIDIA card shows up twice, once from NVML and once
/// from DXGI with PDH engine counters. Keep one entry per card: NVML's
/// temperature, power, fan, clock and memory, with PDH's utilization and
/// engines whenever PDH has a reading, so the card's GPU % is the busiest
/// engine from the same PDH sample as the process list's GPU %. NVML's own
/// utilization (kernel-busy time over its last 1/6 to 1 s) is only a labeled
/// fallback. Cards pair by name only when that name is unique on both sides:
/// identical cards cannot be told apart by name, so none of them borrows
/// another card's PDH reading.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn pair_nvidia(nvml: Vec<GpuDevice>, mut dxgi: Vec<GpuDevice>) -> Vec<GpuDevice> {
    fn key(device: &GpuDevice) -> String {
        device
            .name
            .trim()
            .trim_start_matches("NVIDIA ")
            .trim()
            .to_lowercase()
    }
    let named = |list: &[GpuDevice], name: &str| {
        list.iter()
            .filter(|d| d.vendor == "NVIDIA" && key(d) == name)
            .count()
    };
    let mut cards = Vec::with_capacity(nvml.len() + dxgi.len());
    for mut card in nvml.iter().cloned() {
        let name = key(&card);
        let unique = named(&nvml, &name) == 1 && named(&dxgi, &name) == 1;
        let twin = dxgi
            .iter()
            .position(|d| d.vendor == "NVIDIA" && key(d) == name)
            .map(|index| dxgi.remove(index));
        card.source = match twin {
            Some(twin) if unique => match twin.utilization {
                Some(busy) => {
                    card.utilization = Some(busy);
                    card.engines = twin.engines;
                    "NVIDIA NVML driver; utilization from PDH GPU Engine counters, same sample as the process list"
                }
                None => "NVIDIA NVML driver; utilization from NVML because PDH GPU Engine counters are unavailable",
            },
            Some(_) => "NVIDIA NVML driver; utilization from NVML because several cards share this name",
            None => "NVIDIA NVML driver; utilization from NVML, no PDH adapter with this name",
        }
        .into();
        cards.push(card);
    }
    cards.extend(dxgi);
    cards
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
    fn card(id: &str, name: &str, vendor: &str, utilization: Option<f64>) -> GpuDevice {
        GpuDevice {
            id: id.into(),
            name: name.into(),
            vendor: vendor.into(),
            utilization,
            memory_used: None,
            memory_total: None,
            memory_kind: "dedicated".into(),
            temperature: None,
            power_watts: None,
            core_clock_mhz: None,
            cores: None,
            source: "test".into(),
            fan_percent: None,
            engines: Vec::new(),
        }
    }
    fn nvml(id: &str, name: &str, utilization: Option<f64>) -> GpuDevice {
        GpuDevice {
            memory_used: Some(1 << 30),
            memory_total: Some(8 << 30),
            temperature: Some(51.0),
            power_watts: Some(42.5),
            core_clock_mhz: Some(1800),
            fan_percent: Some(30.0),
            ..card(id, name, "NVIDIA", utilization)
        }
    }
    fn pdh(id: &str, name: &str, vendor: &str, utilization: Option<f64>) -> GpuDevice {
        GpuDevice {
            engines: utilization
                .map(|busy| vec![("3D".into(), busy), ("Copy".into(), 1.0)])
                .unwrap_or_default(),
            ..card(id, name, vendor, utilization)
        }
    }
    fn by_id<'a>(cards: &'a [GpuDevice], id: &str) -> &'a GpuDevice {
        cards.iter().find(|c| c.id == id).unwrap()
    }

    #[test]
    fn an_nvidia_card_shows_the_pdh_busiest_engine_and_keeps_nvml_sensors() {
        let cards = pair_nvidia(
            vec![nvml("GPU-a", "NVIDIA Example 9000", Some(17.0))],
            vec![pdh("luid:0:a", "NVIDIA Example 9000", "NVIDIA", Some(6.5))],
        );
        assert_eq!(cards.len(), 1, "one entry per card");
        let c = &cards[0];
        assert_eq!(c.id, "GPU-a");
        assert_eq!(c.utilization, Some(6.5));
        assert_eq!(c.engines, vec![("3D".into(), 6.5), ("Copy".into(), 1.0)]);
        assert_eq!(
            (c.temperature, c.power_watts, c.memory_used, c.fan_percent),
            (Some(51.0), Some(42.5), Some(1 << 30), Some(30.0))
        );
        assert!(c.source.contains("utilization from PDH"), "{}", c.source);
    }

    #[test]
    fn a_measured_idle_card_reads_zero_and_only_missing_pdh_falls_back_to_nvml() {
        let idle = pair_nvidia(
            vec![nvml("GPU-a", "NVIDIA Example 9000", Some(3.0))],
            vec![pdh("luid:0:a", "NVIDIA Example 9000", "NVIDIA", Some(0.0))],
        );
        assert_eq!(idle[0].utilization, Some(0.0));
        assert!(idle[0].source.contains("from PDH"));

        let missing = pair_nvidia(
            vec![nvml("GPU-a", "NVIDIA Example 9000", Some(3.0))],
            vec![pdh("luid:0:a", "NVIDIA Example 9000", "NVIDIA", None)],
        );
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].utilization, Some(3.0));
        assert!(missing[0].engines.is_empty());
        assert!(missing[0].source.contains("from NVML because PDH"));

        let neither = pair_nvidia(
            vec![nvml("GPU-a", "NVIDIA Example 9000", None)],
            vec![pdh("luid:0:a", "NVIDIA Example 9000", "NVIDIA", None)],
        );
        assert_eq!(neither[0].utilization, None, "unknown, not 0");

        let unmatched = pair_nvidia(
            vec![nvml("GPU-a", "NVIDIA Example 9000", Some(3.0))],
            vec![],
        );
        assert_eq!(unmatched[0].utilization, Some(3.0));
        assert!(unmatched[0].source.contains("no PDH adapter"));
    }

    #[test]
    fn identical_cards_never_borrow_each_others_pdh_reading() {
        let cards = pair_nvidia(
            vec![
                nvml("GPU-a", "NVIDIA Example 9000", Some(10.0)),
                nvml("GPU-b", "NVIDIA Example 9000", Some(90.0)),
            ],
            vec![
                pdh("luid:0:b", "NVIDIA Example 9000", "NVIDIA", Some(88.0)),
                pdh("luid:0:a", "NVIDIA Example 9000", "NVIDIA", Some(9.0)),
            ],
        );
        assert_eq!(cards.len(), 2, "no duplicate DXGI entries");
        for (id, nvml_busy) in [("GPU-a", 10.0), ("GPU-b", 90.0)] {
            let c = by_id(&cards, id);
            assert_eq!(c.utilization, Some(nvml_busy));
            assert!(c.engines.is_empty());
            assert!(c.source.contains("several cards share this name"));
        }
    }

    #[test]
    fn each_adapter_keeps_its_own_reading_on_a_mixed_machine() {
        let cards = pair_nvidia(
            vec![
                nvml("GPU-b", "NVIDIA Example 7000", Some(70.0)),
                nvml("GPU-a", "NVIDIA Example 9000", Some(17.0)),
            ],
            vec![
                pdh("luid:0:1", "Intel(R) Example Graphics", "Intel", Some(12.0)),
                pdh("luid:0:a", "NVIDIA Example 9000", "NVIDIA", Some(6.5)),
                pdh("luid:0:b", "NVIDIA Example 7000", "NVIDIA", Some(2.0)),
            ],
        );
        assert_eq!(cards.len(), 3);
        assert_eq!(by_id(&cards, "GPU-a").utilization, Some(6.5));
        assert_eq!(by_id(&cards, "GPU-b").utilization, Some(2.0));
        let igpu = by_id(&cards, "luid:0:1");
        assert_eq!(
            (igpu.utilization, igpu.source.as_str()),
            (Some(12.0), "test")
        );
    }

    #[test]
    fn unavailable_is_not_idle() {
        assert_eq!(percent(0.0), Some(0.0));
        assert_eq!(percent(100.0), Some(100.0));
        for value in [-1.0, 101.0, f64::NAN, f64::INFINITY] {
            assert_eq!(percent(value), None);
        }
    }
}
