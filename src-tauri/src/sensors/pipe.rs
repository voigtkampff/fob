//! winperf's own read-only sensor helper (`sensors/winperf-sensors.exe`, run
//! elevated by the `winperf-sensors` scheduled task) serves one JSON line per
//! second on `\\.\pipe\winperf-sensors`:
//! `{"ts":<unix ms>,"sensors":[{"hw","id","name","type","value"}]}` with
//! `type` = Temperature | Fan | Control and `value` a number or null.
//! The line format is produced by sensors-helper/src/Wire.cs; both sides are
//! tested against tests/fixtures/sensors-pipe.jsonl.
// Most of this is only reachable on Windows.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use super::{Sensor, SensorKind, SensorSource};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PIPE_PATH: &str = r"\\.\pipe\winperf-sensors";
pub const SOURCE: &str = "winperf sensor helper";

/// Unix ms of the last line received from the helper (0 = never).
static LAST_LINE: AtomicU64 = AtomicU64::new(0);

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// Whether the helper answered within the last few seconds.
pub fn helper_live() -> bool {
    now_ms().saturating_sub(LAST_LINE.load(Ordering::Relaxed)) < 5_000
}

fn kind(text: &str) -> Option<SensorKind> {
    match text {
        "Temperature" => Some(SensorKind::Temperature),
        "Fan" => Some(SensorKind::Fan),
        "Control" => Some(SensorKind::Control),
        _ => None,
    }
}

/// Bytes one process sent/received during `NetSample::ms`.
#[derive(Clone, Debug, PartialEq)]
pub struct NetProc {
    pub pid: u32,
    pub name: Option<String>,
    pub rx: u64,
    pub tx: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetSample {
    pub ms: u64,
    pub procs: Vec<NetProc>,
}

/// What the UI gets: per-process rates in bytes per second.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NetApp {
    pub pid: u32,
    pub name: String,
    pub rx: f64,
    pub tx: f64,
}

/// Bytes per interval -> bytes/s, named with winperf's own process names
/// (same as the process table) and the helper's guess as fallback.
pub fn app_rates(sample: &NetSample, name_of: impl Fn(u32) -> Option<String>) -> Vec<NetApp> {
    let per_second = 1000.0 / sample.ms as f64;
    sample
        .procs
        .iter()
        .map(|p| NetApp {
            pid: p.pid,
            name: name_of(p.pid)
                .or_else(|| p.name.clone())
                .unwrap_or_else(|| format!("PID {}", p.pid)),
            rx: p.rx as f64 * per_second,
            tx: p.tx as f64 * per_second,
        })
        .collect()
}

/// One helper line.
#[derive(Clone, Debug, PartialEq)]
pub struct HelperLine {
    pub ts: u64,
    pub sensors: Vec<Sensor>,
    /// Present while the helper's ETW network session runs.
    pub net: Option<NetSample>,
}

fn parse_net(value: &Value) -> Option<NetSample> {
    let ms = value["ms"].as_u64().filter(|ms| *ms > 0)?;
    let procs = value["procs"]
        .as_array()?
        .iter()
        .filter_map(|p| {
            Some(NetProc {
                pid: u32::try_from(p["pid"].as_u64()?).ok()?,
                name: p["name"].as_str().map(str::to_owned),
                rx: p["rx"].as_u64().unwrap_or(0),
                tx: p["tx"].as_u64().unwrap_or(0),
            })
        })
        .collect();
    Some(NetSample { ms, procs })
}

/// One pipe line. Sensors without a finite value or of another type are
/// dropped; anything that isn't a helper line is `None`.
pub fn parse_line(line: &str) -> Option<HelperLine> {
    let root: Value = serde_json::from_str(line.trim()).ok()?;
    let ts = root["ts"].as_u64()?;
    let sensors = root["sensors"]
        .as_array()?
        .iter()
        .filter_map(|s| {
            Some(Sensor {
                hardware: s["hw"].as_str()?.to_owned(),
                id: s["id"].as_str().unwrap_or_default().to_owned(),
                name: s["name"].as_str()?.to_owned(),
                kind: kind(s["type"].as_str()?)?,
                value: s["value"].as_f64().filter(|v| v.is_finite())?,
                source: SOURCE.into(),
            })
        })
        .collect();
    Some(HelperLine {
        ts,
        sensors,
        net: parse_net(&root["net"]),
    })
}

#[cfg(target_os = "windows")]
pub use client::HelperPipe;

#[cfg(target_os = "windows")]
mod client {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        sync::{Arc, Mutex},
        time::Duration,
    };

    type Latest = Arc<Mutex<Option<(u64, HelperLine)>>>;

    /// Reads the pipe on its own thread so a slow or absent helper never
    /// blocks sampling; `read` hands out the newest line if it is fresh.
    #[derive(Debug, Default)]
    pub struct HelperPipe {
        latest: Latest,
        started: bool,
    }

    fn pump(latest: Latest) {
        let mut last_kick = 0u64;
        loop {
            match std::fs::OpenOptions::new().read(true).open(PIPE_PATH) {
                Ok(pipe) => {
                    for line in BufReader::new(pipe).lines() {
                        let Ok(line) = line else { break };
                        if let Some(parsed) = parse_line(&line) {
                            let now = now_ms();
                            LAST_LINE.store(now, Ordering::Relaxed);
                            *latest.lock().unwrap() = Some((now, parsed));
                        }
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                Err(_) => {
                    // Helper installed but not running (crashed, or the logon
                    // trigger hasn't fired yet): ask the task to start it. The
                    // task runs elevated without a prompt; at most once a minute.
                    let now = now_ms();
                    if now - last_kick > 60_000
                        && crate::sensors::setup::installed_helper().exists()
                    {
                        last_kick = now;
                        crate::sensors::setup::run_task();
                    }
                    std::thread::sleep(Duration::from_secs(3));
                }
            }
        }
    }

    impl SensorSource for HelperPipe {
        fn name(&self) -> &'static str {
            SOURCE
        }
        fn read(&mut self) -> Option<Vec<Sensor>> {
            if !self.started {
                self.started = true;
                let latest = self.latest.clone();
                let _ = std::thread::Builder::new()
                    .name("sensor-pipe".into())
                    .spawn(move || pump(latest));
            }
            let guard = self.latest.lock().ok()?;
            let (at, line) = guard.as_ref()?;
            (now_ms().saturating_sub(*at) < 5_000).then(|| line.sensors.clone())
        }
        fn net(&mut self) -> Option<NetSample> {
            let guard = self.latest.lock().ok()?;
            let (at, line) = guard.as_ref()?;
            (now_ms().saturating_sub(*at) < 5_000).then(|| line.net.clone())?
        }
    }
}

/// Debug builds only: replay a pipe fixture (`WINPERF_FAKE_SENSORS=<file.jsonl>`)
/// through the same parser, one line per read. Used for Linux screenshots; it
/// does not exist in release builds.
#[cfg(debug_assertions)]
#[derive(Debug)]
pub struct FakeReplay {
    lines: Vec<String>,
    next: usize,
    net: Option<NetSample>,
}

#[cfg(debug_assertions)]
impl FakeReplay {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("WINPERF_FAKE_SENSORS")?;
        let text = std::fs::read_to_string(path).ok()?;
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        (!lines.is_empty()).then_some(Self {
            lines,
            next: 0,
            net: None,
        })
    }
}

#[cfg(debug_assertions)]
impl SensorSource for FakeReplay {
    fn name(&self) -> &'static str {
        "winperf sensor helper (debug fixture replay)"
    }
    fn read(&mut self) -> Option<Vec<Sensor>> {
        let line = &self.lines[self.next % self.lines.len()];
        self.next += 1;
        let parsed = parse_line(line)?;
        LAST_LINE.store(now_ms(), Ordering::Relaxed);
        self.net = parsed.net;
        Some(parsed.sensors)
    }
    fn net(&mut self) -> Option<NetSample> {
        self.net.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../../tests/fixtures/sensors-pipe.jsonl");

    #[test]
    fn fixture_lines_parse() {
        let lines: Vec<_> = FIXTURE.lines().collect();
        assert_eq!(lines.len(), 3);
        let HelperLine { ts, sensors, net } = parse_line(lines[0]).unwrap();
        assert_eq!(ts, 1_793_612_400_000);
        // 19 written, Load is never written; null/NaN/Infinity values dropped.
        assert_eq!(sensors.len(), 15);
        let package = sensors.iter().find(|s| s.name == "CPU Package").unwrap();
        assert_eq!(package.hardware, "Example Hybrid CPU");
        assert_eq!(package.id, "/intelcpu/0/temperature/16");
        assert_eq!(
            (package.kind, package.value),
            (SensorKind::Temperature, 52.0)
        );
        let fans: Vec<_> = sensors
            .iter()
            .filter(|s| s.kind == SensorKind::Fan)
            .collect();
        assert_eq!(fans.len(), 4); // Fan #1, #2, #3 (0 RPM) and GPU Fan 1; Fan #7 was NaN
        assert!(sensors.iter().all(|s| s.source == SOURCE));
        let duty = sensors
            .iter()
            .find(|s| s.kind == SensorKind::Control)
            .unwrap();
        assert_eq!(duty.value, 42.35);
        assert!(!sensors.iter().any(|s| s.hardware.starts_with("Weird")));
        let net = net.unwrap();
        assert_eq!(net.ms, 1000);
        assert_eq!(net.procs.len(), 6);
        assert_eq!(
            net.procs[1],
            NetProc {
                pid: 10412,
                name: Some("chrome".into()),
                rx: 1_200_000,
                tx: 35_000
            }
        );
        let later = parse_line(lines[2]).unwrap().sensors;
        let package = later.iter().find(|s| s.name == "CPU Package").unwrap();
        assert_eq!(package.value, 54.0);
    }

    #[test]
    fn rates_are_per_second_and_prefer_winperf_names() {
        let sample = NetSample {
            ms: 500,
            procs: vec![
                NetProc {
                    pid: 1,
                    name: Some("chrome".into()),
                    rx: 1000,
                    tx: 10,
                },
                NetProc {
                    pid: 2,
                    name: Some("svchost".into()),
                    rx: 0,
                    tx: 50,
                },
                NetProc {
                    pid: 3,
                    name: None,
                    rx: 5,
                    tx: 0,
                },
            ],
        };
        let apps = app_rates(&sample, |pid| (pid == 1).then(|| "chrome.exe".to_string()));
        assert_eq!(apps[0].name, "chrome.exe");
        assert_eq!((apps[0].rx, apps[0].tx), (2000.0, 20.0));
        assert_eq!(apps[1].name, "svchost");
        assert_eq!(apps[2].name, "PID 3");
    }

    #[test]
    fn junk_is_rejected_and_bad_entries_skipped() {
        assert!(parse_line("").is_none());
        assert!(parse_line("{\"sensors\":[]}").is_none());
        assert!(parse_line("[1,2]").is_none());
        let HelperLine {
            sensors: s, net, ..
        } = parse_line(
            r#"{"ts":1,"sensors":[{"hw":"A","id":"/a/0/fan/0","name":"F","type":"Fan","value":"12"},
               {"hw":"A","name":"T","type":"Temperature","value":30.5},
               {"hw":"A","id":"x","name":"V","type":"Voltage","value":1.2}]}"#,
        )
        .unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].name.as_str(), s[0].id.as_str()), ("T", ""));
        assert!(net.is_none()); // a helper without ETW sends no "net"
        let bad = parse_line(
            r#"{"ts":1,"sensors":[],"net":{"ms":500,"procs":[{"pid":-1},{"pid":7,"rx":10}]}}"#,
        )
        .unwrap()
        .net
        .unwrap();
        assert_eq!(
            bad.procs,
            vec![NetProc {
                pid: 7,
                name: None,
                rx: 10,
                tx: 0
            }]
        );
        assert!(
            parse_line(r#"{"ts":1,"sensors":[],"net":{"ms":0,"procs":[]}}"#)
                .unwrap()
                .net
                .is_none()
        );
    }
}
