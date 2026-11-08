//! Run directory layout: run.json, samples.jsonl, processes.csv, intervals.csv,
//! summary.json, summary.txt. Never overwrites an existing directory.
use crate::analyze::{intervals, render, summarize, Summary};
use crate::model::{RunMeta, Sample, Role, RootState, SCHEMA, SCOPE, MODES};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub fn create_new_dir(p: &Path) -> Result<(), String> {
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    fs::create_dir(p).map_err(|e| format!("cannot create new result directory {}: {e}", p.display()))
}

fn write_new(p: &Path, data: &[u8]) -> Result<(), String> {
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    f.write_all(data).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn write_meta(dir: &Path, m: &RunMeta) -> Result<(), String> {
    write_new(&dir.join("run.json"), serde_json::to_string_pretty(m).unwrap().as_bytes())
}

pub fn read_run(dir: &Path) -> Result<(RunMeta, Vec<Sample>, Option<(f64, f64)>), String> {
    let meta: RunMeta = serde_json::from_slice(&fs::read(dir.join("run.json")).map_err(|e| format!("{}/run.json: {e}", dir.display()))?)
        .map_err(|e| format!("{}/run.json: {e}", dir.display()))?;
    validate_meta(&meta)?;
    let f = fs::File::open(dir.join("samples.jsonl")).map_err(|e| format!("{}/samples.jsonl: {e}", dir.display()))?;
    let mut samples = vec![];
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        samples.push(serde_json::from_str(&line).map_err(|e| format!("samples.jsonl line {}: {e}", i + 1))?);
    }
    validate_samples(&meta, &samples)?;
    let sampler_path = dir.join("sampler.json");
    let sampler = match fs::read(&sampler_path) {
        Ok(data) => Some(serde_json::from_slice(&data).map_err(|e| format!("sampler.json: {e}"))?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("sampler.json: {e}")),
    };
    Ok((meta, samples, sampler))
}

/// Derive every report file from raw data. `analyze` on an existing run writes
/// into a new sibling directory instead of replacing files.
pub fn write_reports(dir: &Path, meta: RunMeta, samples: &[Sample], sampler: Option<(f64, f64)>) -> Result<Summary, String> {
    validate_meta(&meta)?;
    validate_samples(&meta, samples)?;
    let s = summarize(meta, samples, sampler);
    if let Some(u) = sampler {
        write_new(&dir.join("sampler.json"), serde_json::to_string(&u).unwrap().as_bytes())?;
    }
    let mut pc = String::from("seq,t_s,cpu_read_t_s,pid,create_filetime,role,name,cpu_100ns,private_bytes,working_set_bytes,error\n");
    for x in samples {
        for o in &x.procs {
            let n = |v: Option<u64>| v.map_or(String::new(), |v| v.to_string());
            pc += &format!("{},{:.3},{:.6},{},{},{:?},{},{},{},{},{}\n", x.seq, x.t_ns as f64 / 1e9, o.cpu_t_ns as f64 / 1e9, o.id.pid, o.id.create, o.role, csv(&o.name), n(o.cpu), n(o.private_bytes), n(o.working_set), csv(o.error.as_deref().unwrap_or("")));
        }
    }
    write_new(&dir.join("processes.csv"), pc.as_bytes())?;
    let mut ic = String::from("seq,start_s,elapsed_s,cpu_pct_all_cores,processes,born,exited,missing,late\n");
    for i in intervals(samples, s.meta.logical_cores, s.meta.interval_ms) {
        ic += &format!("{},{:.3},{:.4},{},{},{},{},{},{}\n", i.seq, i.start_s, i.elapsed_s, i.cpu_pct.map_or(String::new(), |v| format!("{v:.4}")), i.processes, i.born, i.exited, i.missing, i.late);
    }
    write_new(&dir.join("intervals.csv"), ic.as_bytes())?;
    write_new(&dir.join("summary.json"), serde_json::to_string_pretty(&s).unwrap().as_bytes())?;
    write_new(&dir.join("summary.txt"), render(&s).as_bytes())?;
    Ok(s)
}

pub fn read_summary(dir: &Path) -> Result<Summary, String> {
    let p = dir.join("summary.json");
    let saved: Summary = serde_json::from_slice(&fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?).map_err(|e| format!("{}: {e}", p.display()))?;
    let (meta, samples, sampler) = read_run(dir)?;
    let derived = summarize(meta, &samples, sampler);
    if saved != derived { return Err(format!("{}: summary disagrees with raw run data", dir.display())); }
    Ok(derived)
}

pub fn validate_meta(m: &RunMeta) -> Result<(), String> {
    if m.schema != SCHEMA { return Err(format!("schema {} not supported", m.schema)); }
    if m.logical_cores == 0 || m.processor_groups == 0 || m.machine.is_empty()
        || !(250..=10_000).contains(&m.interval_ms) || m.refresh_ms == 0
        || m.steady_s == 0 || m.settle_s.checked_add(m.steady_s).is_none()
        || m.settle_s + m.steady_s > 604800 || !MODES.contains(&m.mode.as_str())
        || m.scope != SCOPE || !m.root_exe_name.eq_ignore_ascii_case("fob.exe")
        || m.root.pid == 0 || m.root.create == 0 || m.run_id.is_empty()
        || m.label.is_empty() || m.app_version.trim().is_empty() || m.helper.trim().is_empty()
        || !m.root_age_at_attach_s.is_finite() || m.root_age_at_attach_s < 0.0
        || m.started_utc.strip_prefix("unix:").and_then(|s| s.parse::<u64>().ok()).is_none() {
        return Err("invalid run metadata, scope or declared settings".into());
    }
    if m.root_exe_sha256.as_ref().is_some_and(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
        || m.root_exe_sha256.is_some() != m.root_exe_bytes.is_some()
        || m.root_exe_bytes == Some(0) {
        return Err("invalid executable metadata".into());
    }
    let mut keys = std::collections::HashSet::new();
    for s in &m.settings {
        let (k, v) = s.split_once('=').ok_or("setting must be KEY=VALUE")?;
        if k.trim().is_empty() || v.trim().is_empty() || !keys.insert(k) {
            return Err("empty or duplicate setting".into());
        }
    }
    Ok(())
}

pub fn validate_samples(m: &RunMeta, samples: &[Sample]) -> Result<(), String> {
    for w in samples.windows(2) {
        if w[1].seq <= w[0].seq || w[1].t_ns <= w[0].t_ns {
            return Err("sample sequence and monotonic timestamps must increase".into());
        }
    }
    for s in samples {
        let mut ids = std::collections::HashSet::new();
        let mut roots = 0;
        for o in &s.procs {
            if !ids.insert(o.id.pid) { return Err("duplicate process in sample".into()); }
            match o.role {
                Role::Root => {
                    roots += 1;
                    if o.id != m.root || !o.name.eq_ignore_ascii_case(&m.root_exe_name) {
                        return Err("sample root disagrees with run metadata".into());
                    }
                }
                Role::Child if !o.name.eq_ignore_ascii_case("msedgewebview2.exe") => {
                    return Err("sample contains a non-WebView descendant".into());
                }
                Role::Child => {}
            }
        }
        if roots > 1 || (s.root == RootState::Alive && roots != 1) {
            return Err("sample is missing its bound root".into());
        }
    }
    Ok(())
}

fn csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
