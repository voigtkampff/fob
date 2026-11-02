//! winperf: temperatures and fan speeds from sources that need no driver of our
//! own. Each source sits behind `SensorSource` so it can be swapped (e.g. a
//! future FanControl reader) without touching the sampler or the UI.
use crate::gpu::GpuDevice;
use serde::Serialize;
use std::fmt::Debug;

pub mod lhm;

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
}

/// A place sensor readings come from. `None` = source not available right now.
pub trait SensorSource: Send + Debug {
    fn name(&self) -> &'static str;
    fn read(&mut self) -> Option<Vec<Sensor>>;
}

/// NVIDIA GPU temperature / fan duty, read through NeoHtop's NVML GPU code.
pub fn from_gpus(gpus: &[GpuDevice]) -> Vec<Sensor> {
    let mut out = Vec::new();
    for gpu in gpus.iter().filter(|g| g.source.contains("NVML")) {
        if let Some(t) = gpu.temperature.filter(|t| t.is_finite()) {
            out.push(Sensor {
                hardware: gpu.name.clone(),
                name: "GPU core".into(),
                kind: SensorKind::Temperature,
                value: t,
                source: "NVML".into(),
            });
        }
        if let Some(f) = gpu.fan_percent.filter(|f| f.is_finite()) {
            out.push(Sensor {
                hardware: gpu.name.clone(),
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
        Self {
            // LibreHardwareMonitor's own web server is Windows-only in practice;
            // don't probe localhost ports on other platforms.
            sources: if cfg!(target_os = "windows") {
                vec![Box::new(lhm::LhmHttp::default())]
            } else {
                Vec::new()
            },
        }
    }
}

impl SensorHub {
    pub fn read(&mut self, gpus: &[GpuDevice]) -> SensorReport {
        let mut report = SensorReport::default();
        for source in &mut self.sources {
            if let Some(sensors) = source.read() {
                report.sources.push(source.name().into());
                report.sensors.extend(sensors);
            }
        }
        let nvml = from_gpus(gpus);
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
        assert_eq!(r.sensors, vec![fan]);
    }
}
