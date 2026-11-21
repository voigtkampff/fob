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
    Temperature,
    Fan,
    Control,
    Voltage,
    Power,
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

/// MSR-derived throttle state from the helper (0.8.5+). `supported: false`
/// means the machine/driver/helper could not read the MSRs, so the UI shows
/// "not supported" instead of guessing.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThrottleReport {
    pub supported: bool,
    pub thermal: bool,
    pub thermal_log: bool,
    pub prochot: bool,
    pub prochot_log: bool,
    /// MSR 0x613 advanced during the helper's last interval.
    pub package: bool,
    /// Hottest core/package distance to TjMax, °C. Serde's camelCase would
    /// write "distanceToTjmax"; the UI and wire both say "distanceToTjMax".
    #[serde(rename = "distanceToTjMax")]
    pub distance_to_tjmax: Option<i32>,
    pub thermal_percent: f64,
    pub package_percent: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SensorReport {
    pub sensors: Vec<Sensor>,
    pub sources: Vec<String>,
    pub throttle: Option<ThrottleReport>,
    /// Per-process network bytes from the helper's ETW session (not sent to
    /// the UI as is; `sampling` turns it into named per-app rates).
    #[serde(skip)]
    pub net: Option<pipe::NetSample>,
}

pub trait SensorSource: Send + Debug {
    fn name(&self) -> &'static str;
    fn read(&mut self) -> Option<Vec<Sensor>>;
    fn net(&mut self) -> Option<pipe::NetSample> {
        None
    }
    fn throttle(&mut self) -> Option<ThrottleReport> {
        None
    }
}

/// Which device a sensor belongs to. LibreHardwareMonitor ids start with the
/// hardware type ("/amdcpu/0/…", "/gpu-nvidia/0/…"), so an APU named
/// "… w/ Radeon 780M Graphics" is still a CPU; NVML ids start "nvml/".
/// Without an id, the hardware name decides, CPU words first. Same rules as
/// sensorDevice() in src/lib/utils/dashboard.js.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Cpu,
    Gpu,
    Other,
}

pub fn device(s: &Sensor) -> Device {
    let id = s.id.to_lowercase();
    if !id.is_empty() {
        let kind = id
            .trim_start_matches('/')
            .split('/')
            .next()
            .unwrap_or_default();
        return match kind {
            "amdcpu" | "intelcpu" | "cpu" => Device::Cpu,
            "nvidiagpu" | "atigpu" | "nvml" => Device::Gpu,
            k if k.starts_with("gpu-") => Device::Gpu,
            _ => Device::Other,
        };
    }
    let hw = s.hardware.to_lowercase();
    let word = |w: &str| hw.split(|c: char| !c.is_alphanumeric()).any(|x| x == w);
    if hw.contains("ryzen")
        || hw.contains("intel core")
        || hw.contains("core i")
        || hw.contains("core ultra")
        || word("cpu")
        || hw.contains("processor")
    {
        Device::Cpu
    } else if ["gpu", "nvidia", "geforce", "radeon"]
        .iter()
        .any(|w| hw.contains(w))
        || word("arc")
    {
        Device::Gpu
    } else {
        Device::Other
    }
}

fn is_non_core_rail(name: &str) -> bool {
    [
        "termination",
        "vccio",
        "vccsa",
        "vccin",
        "vddq",
        "vtt",
        "pll",
        "soc",
    ]
    .iter()
    .any(|word| name.contains(word))
}

/// Whether a voltage reading is the CPU's core supply: its own hardware
/// (`/intelcpu/`, `/amdcpu/`, `/cpu/`) or the Super I/O channel a board names
/// after the CPU ("Vcore"). Every other channel of a board is a whole-board
/// rail (AVCC, +3.3V, 5V, 12V, "Voltage #7"), and a rail in
/// `is_non_core_rail` ("CPU Termination") is not the core supply either.
/// Same rule as Wire.Wanted in sensors-helper/src/Wire.cs.
pub fn is_cpu_voltage(s: &Sensor) -> bool {
    let name = s.name.to_lowercase();
    if is_non_core_rail(&name) {
        return false;
    }
    let id = s.id.to_lowercase();
    if ["/intelcpu/", "/amdcpu/", "/cpu/"]
        .iter()
        .any(|prefix| id.starts_with(prefix))
    {
        return true;
    }
    name.contains("vcore") || name.contains("cpu")
}

/// Whether a power reading is the CPU package's. LHM reports power in W; the
/// helper sends only this sensor, and the hub drops anything else (a GPU's
/// power comes from NVML, not from here). Same rule as Wire.Wanted.
pub fn is_cpu_power(s: &Sensor) -> bool {
    device(s) == Device::Cpu && s.name.to_lowercase().contains("package")
}

/// NVIDIA GPU temperature / fan duty, read through NeoHtop's NVML GPU code.
pub fn from_gpus(gpus: &[GpuDevice]) -> Vec<Sensor> {
    let mut out = Vec::new();
    for gpu in gpus
        .iter()
        .filter(|g| crate::gpu::is_nvml_source(&g.source))
    {
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
                report
                    .sensors
                    .extend(sensors.into_iter().filter(|s| match s.kind {
                        SensorKind::Voltage => is_cpu_voltage(s),
                        SensorKind::Power => is_cpu_power(s),
                        _ => true,
                    }));
                report.net = source.net();
                report.throttle = source.throttle();
                break;
            }
        }
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
            gpu(crate::gpu::SOURCE_NVML, Some(48.0), Some(30.0)),
            gpu(crate::gpu::SOURCE_DXGI, Some(40.0), None),
            gpu(crate::gpu::SOURCE_DXGI_PDH, Some(40.0), None),
            gpu(crate::gpu::SOURCE_NVML, None, Some(f64::NAN)),
        ]);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].kind, s[0].value), (SensorKind::Temperature, 48.0));
        assert_eq!((s[1].kind, s[1].value), (SensorKind::Control, 30.0));
        let paired = from_gpus(&[gpu(crate::gpu::SOURCE_NVML_PDH, Some(51.0), Some(20.0))]);
        assert_eq!(paired.len(), 2);
        assert_eq!(paired[0].value, 51.0);
    }
    #[test]
    fn cpu_voltage_is_the_only_voltage_fob_shows() {
        let s = |id: &str, name: &str| Sensor {
            hardware: "ASUS PRIME Z790-P D4".into(),
            id: id.into(),
            name: name.into(),
            kind: SensorKind::Voltage,
            value: 1.24,
            source: "test".into(),
        };
        assert!(is_cpu_voltage(&s("/lpc/nct6798d/0/voltage/0", "Vcore")));
        assert!(!is_cpu_voltage(&s(
            "/lpc/nct6798d/0/voltage/9",
            "CPU Termination"
        )));
        assert!(!is_cpu_voltage(&s(
            "/lpc/nct6798d/0/voltage/10",
            "CPU VCCIO"
        )));
        assert!(!is_cpu_voltage(&s("/amdcpu/0/voltage/1", "SoC (SVI2 TFN)")));
        assert!(is_cpu_voltage(&s("/intelcpu/0/voltage/0", "CPU Core")));
        assert!(is_cpu_voltage(&s("/intelcpu/0/voltage/8", "P-Core #8")));
        assert!(is_cpu_voltage(&s("/amdcpu/0/voltage/0", "Core (SVI2 TFN)")));
        assert!(!is_cpu_voltage(&s("/lpc/nct6798d/0/voltage/2", "AVCC")));
        assert!(!is_cpu_voltage(&s("/lpc/nct6798d/0/voltage/3", "+3.3V")));
        assert!(!is_cpu_voltage(&s("/lpc/nct6798d/0/voltage/7", "+12V")));
        assert!(!is_cpu_voltage(&s(
            "/gpu-nvidia/0/voltage/0",
            "GPU Core Voltage"
        )));
    }
    #[test]
    fn only_the_cpu_package_power_is_kept() {
        let s = |id: &str, name: &str| Sensor {
            hardware: "Example".into(),
            id: id.into(),
            name: name.into(),
            kind: SensorKind::Power,
            value: 85.0,
            source: "test".into(),
        };
        assert!(is_cpu_power(&s("/intelcpu/0/power/0", "CPU Package")));
        assert!(is_cpu_power(&s("/amdcpu/0/power/2", "Package")));
        assert!(!is_cpu_power(&s("/intelcpu/0/power/1", "CPU Cores")));
        assert!(!is_cpu_power(&s("/gpu-nvidia/0/power/0", "GPU Package")));
    }
    #[test]
    fn hub_drops_whole_board_rails_and_other_power() {
        let gpu_power = Sensor {
            hardware: "NVIDIA Example GPU".into(),
            id: "/gpu-nvidia/0/power/0".into(),
            name: "GPU Package".into(),
            kind: SensorKind::Power,
            value: 200.0,
            source: "Fixed".into(),
        };
        let cpu_power = Sensor {
            hardware: "Example Hybrid CPU".into(),
            id: "/intelcpu/0/power/0".into(),
            name: "CPU Package".into(),
            kind: SensorKind::Power,
            value: 85.3,
            source: "Fixed".into(),
        };
        let mut power_hub = SensorHub {
            sources: vec![Box::new(Fixed(Some(vec![gpu_power, cpu_power.clone()])))],
        };
        assert_eq!(power_hub.read(&[]).sensors, vec![cpu_power]);
        let rail = Sensor {
            hardware: "Example Board".into(),
            id: "/lpc/example/0/voltage/3".into(),
            name: "+3.3V".into(),
            kind: SensorKind::Voltage,
            value: 3.328,
            source: "Fixed".into(),
        };
        let vcore = Sensor {
            id: "/lpc/example/0/voltage/0".into(),
            name: "Vcore".into(),
            value: 1.24,
            ..rail.clone()
        };
        let mut hub = SensorHub {
            sources: vec![Box::new(Fixed(Some(vec![rail, vcore.clone()])))],
        };
        assert_eq!(hub.read(&[]).sensors, vec![vcore]);
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
        let nvml = gpu(crate::gpu::SOURCE_NVML, Some(48.0), Some(30.0));
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
