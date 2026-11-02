//! winperf: temperatures and fan speeds. Each source sits behind `SensorSource`
//! so it can be swapped without touching the sampler or the UI. Board sources
//! are tried in order and the first that answers wins: winperf's own helper
//! pipe, then LibreHardwareMonitor's web server. NVIDIA NVML is always merged.
use crate::gpu::GpuDevice;
use serde::Serialize;
use std::fmt::Debug;

pub mod lhm;
pub mod pipe;
pub mod setup;
#[cfg(target_os = "windows")]
mod task;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SensorKind {
    /// °C
    Temperature,
    /// RPM
    Fan,
    /// Fan duty, %
    Control,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Sensor {
    pub hardware: String,
    /// Source-specific identifier, e.g. "/lpc/example/0/fan/1" ("" if none).
    pub id: String,
    pub name: String,
    pub kind: SensorKind,
    pub value: f64,
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SensorReport {
    pub sensors: Vec<Sensor>,
    /// Names of the sources that answered this round.
    pub sources: Vec<String>,
    /// Per-process network bytes from the helper's ETW session (not sent to
    /// the UI as is; `sampling` turns it into named per-app rates).
    #[serde(skip)]
    pub net: Option<pipe::NetSample>,
}

/// A place sensor readings come from. `None` = source not available right now.
pub trait SensorSource: Send + Debug {
    fn name(&self) -> &'static str;
    fn read(&mut self) -> Option<Vec<Sensor>>;
    /// Per-process network bytes that came with the last `read`, if any.
    fn net(&mut self) -> Option<pipe::NetSample> {
        None
    }
}

/// NVIDIA GPU temperature / fan duty, read through NeoHtop's NVML GPU code.
pub fn from_gpus(gpus: &[GpuDevice]) -> Vec<Sensor> {
    let mut out = Vec::new();
    for gpu in gpus.iter().filter(|g| g.source.contains("NVML")) {
        if let Some(t) = gpu.temperature.filter(|t| t.is_finite()) {
            out.push(Sensor {
                hardware: gpu.name.clone(),
                id: format!("nvml/{}/temperature", gpu.id),
                name: "GPU core".into(),
                kind: SensorKind::Temperature,
                value: t,
                source: "NVML".into(),
            });
        }
        if let Some(f) = gpu.fan_percent.filter(|f| f.is_finite()) {
            out.push(Sensor {
                hardware: gpu.name.clone(),
                id: format!("nvml/{}/fan", gpu.id),
                name: "GPU fan".into(),
                kind: SensorKind::Control,
                value: f,
                source: "NVML".into(),
            });
        }
    }
    out
}

#[derive(Debug)]
pub struct SensorHub {
    sources: Vec<Box<dyn SensorSource>>,
}

impl Default for SensorHub {
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut sources: Vec<Box<dyn SensorSource>> = Vec::new();
        #[cfg(debug_assertions)]
        if let Some(fake) = pipe::FakeReplay::from_env() {
            sources.push(Box::new(fake));
        }
        // Both board sources are Windows-only; don't probe pipes or localhost
        // ports elsewhere.
        #[cfg(target_os = "windows")]
        {
            sources.push(Box::new(pipe::HelperPipe::default()));
            sources.push(Box::new(lhm::LhmHttp::default()));
        }
        Self { sources }
    }
}

impl SensorHub {
    pub fn read(&mut self, gpus: &[GpuDevice]) -> SensorReport {
        let mut report = SensorReport::default();
        for source in &mut self.sources {
            if let Some(sensors) = source.read() {
                report.sources.push(source.name().into());
                report.sensors.extend(sensors);
                report.net = source.net();
                break;
            }
        }
        // NVML only for GPUs the board source didn't already cover.
        let covered = |name: &str| {
            report
                .sensors
                .iter()
                .any(|s| s.hardware.eq_ignore_ascii_case(name))
        };
        let nvml: Vec<Sensor> = from_gpus(gpus)
            .into_iter()
            .filter(|s| !covered(&s.hardware))
            .collect();
        if !nvml.is_empty() {
            report.sources.push("NVML".into());
            report.sensors.extend(nvml);
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gpu(source: &str, temperature: Option<f64>, fan: Option<f64>) -> GpuDevice {
        GpuDevice {
            id: "x".into(),
            name: "NVIDIA Example GPU".into(),
            vendor: "NVIDIA".into(),
            utilization: None,
            memory_used: None,
            memory_total: None,
            memory_kind: "dedicated".into(),
            temperature,
            power_watts: None,
            core_clock_mhz: None,
            cores: None,
            source: source.into(),
            fan_percent: fan,
            engines: Vec::new(),
        }
    }
    #[test]
    fn nvml_gpus_become_sensors_and_others_do_not() {
        let s = from_gpus(&[
            gpu("NVIDIA NVML driver", Some(48.0), Some(30.0)),
            gpu("Windows DXGI inventory", Some(40.0), None),
            gpu("NVIDIA NVML driver", None, Some(f64::NAN)),
        ]);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].kind, s[0].value), (SensorKind::Temperature, 48.0));
        assert_eq!((s[1].kind, s[1].value), (SensorKind::Control, 30.0));
    }
    #[derive(Debug)]
    struct Fixed(Option<Vec<Sensor>>);
    impl SensorSource for Fixed {
        fn name(&self) -> &'static str {
            "Fixed"
        }
        fn read(&mut self) -> Option<Vec<Sensor>> {
            self.0.clone()
        }
    }
    #[test]
    fn hub_merges_available_sources_only() {
        let fan = Sensor {
            hardware: "Board".into(),
            id: "/lpc/x/0/fan/0".into(),
            name: "Fan #1".into(),
            kind: SensorKind::Fan,
            value: 900.0,
            source: "Fixed".into(),
        };
        let mut hub = SensorHub {
            sources: vec![
                Box::new(Fixed(None)),
                Box::new(Fixed(Some(vec![fan.clone()]))),
            ],
        };
        let r = hub.read(&[]);
        assert_eq!(r.sources, vec!["Fixed".to_string()]);
        assert_eq!(r.sensors, vec![fan.clone()]);
    }
    #[test]
    fn first_board_source_wins_and_nvml_fills_uncovered_gpus() {
        let gpu_temp = Sensor {
            hardware: "NVIDIA Example GPU".into(),
            id: "/gpu-nvidia/0/temperature/0".into(),
            name: "GPU Core".into(),
            kind: SensorKind::Temperature,
            value: 48.5,
            source: "Fixed".into(),
        };
        let other = Sensor {
            name: "Other".into(),
            ..gpu_temp.clone()
        };
        let mut hub = SensorHub {
            sources: vec![
                Box::new(Fixed(Some(vec![gpu_temp.clone()]))),
                Box::new(Fixed(Some(vec![other]))),
            ],
        };
        let nvml = gpu("NVIDIA NVML driver", Some(48.0), Some(30.0));
        let r = hub.read(std::slice::from_ref(&nvml));
        assert_eq!(r.sensors, vec![gpu_temp]);
        assert_eq!(r.sources, vec!["Fixed".to_string()]);
        let mut empty = SensorHub {
            sources: vec![Box::new(Fixed(None))],
        };
        let r = empty.read(&[nvml]);
        assert_eq!(r.sources, vec!["NVML".to_string()]);
        assert_eq!(r.sensors.len(), 2);
    }
}
