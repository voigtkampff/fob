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

/// One pipe line -> (timestamp, sensors). Sensors without a finite value or of
/// another type are dropped; anything that isn't a helper line is `None`.
pub fn parse_line(line: &str) -> Option<(u64, Vec<Sensor>)> {
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
    Some((ts, sensors))
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

    type Latest = Arc<Mutex<Option<(u64, Vec<Sensor>)>>>;

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
                        if let Some((_, sensors)) = parse_line(&line) {
                            let now = now_ms();
                            LAST_LINE.store(now, Ordering::Relaxed);
                            *latest.lock().unwrap() = Some((now, sensors));
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
            let (at, sensors) = guard.as_ref()?;
            (now_ms().saturating_sub(*at) < 5_000).then(|| sensors.clone())
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
}

#[cfg(debug_assertions)]
impl FakeReplay {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("WINPERF_FAKE_SENSORS")?;
        let text = std::fs::read_to_string(path).ok()?;
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        (!lines.is_empty()).then_some(Self { lines, next: 0 })
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
        let (_, sensors) = parse_line(line)?;
        LAST_LINE.store(now_ms(), Ordering::Relaxed);
        Some(sensors)
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
        let (ts, sensors) = parse_line(lines[0]).unwrap();
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
        let (_, later) = parse_line(lines[2]).unwrap();
        let package = later.iter().find(|s| s.name == "CPU Package").unwrap();
        assert_eq!(package.value, 54.0);
    }

    #[test]
    fn junk_is_rejected_and_bad_entries_skipped() {
        assert!(parse_line("").is_none());
        assert!(parse_line("{\"sensors\":[]}").is_none());
        assert!(parse_line("[1,2]").is_none());
        let (_, s) = parse_line(
            r#"{"ts":1,"sensors":[{"hw":"A","id":"/a/0/fan/0","name":"F","type":"Fan","value":"12"},
               {"hw":"A","name":"T","type":"Temperature","value":30.5},
               {"hw":"A","id":"x","name":"V","type":"Voltage","value":1.2}]}"#,
        )
        .unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].name.as_str(), s[0].id.as_str()), ("T", ""));
    }
}
