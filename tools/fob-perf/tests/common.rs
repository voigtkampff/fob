#![allow(dead_code)]
use fob_perf::collect::{self, Read, Reader};
use fob_perf::model::*;
use std::collections::HashMap;

/// A deterministic fake OS: process list plus per-pid counters.
#[derive(Default, Clone)]
pub struct Os {
    pub procs: Vec<(u32, u32, &'static str)>,
    pub info: HashMap<u32, (u64, u64, Option<u64>, u64)>, // create, cpu, private, ws
    pub denied: Vec<u32>,
    pub list_fails: bool,
}

impl Reader for Os {
    fn create_time(&mut self, pid: u32) -> Result<u64, String> {
        if self.denied.contains(&pid) {
            return Err("OpenProcess: Access is denied. (os error 5)".into());
        }
        self.info.get(&pid).map(|i| i.0).ok_or("gone".into())
    }
    fn read(&mut self, pid: u32) -> Result<Read, String> {
        let i = self.info.get(&pid).ok_or("gone")?;
        Ok(Read { create: i.0, cpu: i.1, private_bytes: i.2, working_set: Some(i.3) })
    }
}

impl Os {
    pub fn add(&mut self, pid: u32, ppid: u32, name: &'static str, create: u64, private_mb: u64) {
        self.procs.push((pid, ppid, name));
        self.info.insert(pid, (create, 0, Some(private_mb << 20), (private_mb + 10) << 20));
    }
    pub fn kill(&mut self, pid: u32) {
        self.procs.retain(|p| p.0 != pid);
        self.info.remove(&pid);
    }
    pub fn burn(&mut self, pid: u32, cpu_100ns: u64) {
        self.info.get_mut(&pid).unwrap().1 += cpu_100ns;
    }
    pub fn entries(&self) -> Result<Vec<Entry>, String> {
        if self.list_fails {
            return Err("CreateToolhelp32Snapshot failed".into());
        }
        Ok(self.procs.iter().map(|p| Entry { pid: p.0, ppid: p.1, name: p.2.into() }).collect())
    }
}

pub const WALL0: u64 = 133_000_000_000_000_000;
pub const SEC: u64 = 10_000_000; // FILETIME units

/// Typical Fob: root 100 + two WebView children, unrelated WebView, helper.
pub fn fob() -> Os {
    let mut os = Os::default();
    os.add(4, 0, "System", 1, 0);
    os.add(50, 4, "explorer.exe", WALL0 - 1000 * SEC, 80);
    os.add(100, 50, "fob.exe", WALL0 - 300 * SEC, 120);
    os.add(110, 100, "msedgewebview2.exe", WALL0 - 299 * SEC, 90);
    os.add(111, 110, "msedgewebview2.exe", WALL0 - 299 * SEC, 60);
    os.add(200, 50, "msedgewebview2.exe", WALL0 - 500 * SEC, 300); // another app
    os.add(300, 4, "fob-sensors.exe", WALL0 - 900 * SEC, 40);
    os
}

pub fn root(os: &Os) -> Identity {
    Identity { pid: 100, create: os.info[&100].0 }
}

pub fn meta(label: &str, mode: &str, settle: u64, steady: u64, cores: u32) -> RunMeta {
    RunMeta {
        schema: SCHEMA,
        run_id: format!("{label}-{mode}"),
        tool_version: "test".into(),
        label: label.into(),
        app_version: "0.8.2".into(),
        mode: mode.into(),
        refresh_ms: 2000,
        helper: "current".into(),
        settings: vec!["theme=dark".into()],
        interval_ms: 1000,
        settle_s: settle,
        steady_s: steady,
        logical_cores: cores,
        processor_groups: 1,
        machine: "m1".into(),
        started_utc: "unix:000000000001".into(),
        root: Identity { pid: 100, create: WALL0 - 300 * SEC },
        root_exe_name: "fob.exe".into(),
        root_exe_sha256: Some("aa".into()),
        root_exe_bytes: Some(1),
        root_age_at_attach_s: 300.0,
        scope: SCOPE.into(),
    }
}

/// Run `n` one-second samples; `step(i, os)` mutates the OS before sample i.
pub fn run(os: &mut Os, n: u64, mut step: impl FnMut(u64, &mut Os)) -> Vec<Sample> {
    let r = root(os);
    let mut out = vec![];
    for i in 0..n {
        step(i, os);
        let ents = os.entries();
        let (s, _) = collect::sample(i, i * 1_000_000_000, WALL0 + i * SEC, r, ents, os);
        out.push(s);
    }
    out
}
