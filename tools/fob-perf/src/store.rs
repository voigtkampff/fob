//! Run directory layout: run.json, samples.jsonl, processes.csv, intervals.csv,
//! summary.json, summary.txt. Never overwrites an existing directory.
use crate::analyze::{intervals, render, summarize, Summary};
use crate::model::{RunMeta, Sample, SCHEMA};
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
    if meta.schema != SCHEMA {
        return Err(format!("{}: schema {} not supported", dir.display(), meta.schema));
    }
    let f = fs::File::open(dir.join("samples.jsonl")).map_err(|e| format!("{}/samples.jsonl: {e}", dir.display()))?;
    let mut samples = vec![];
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        samples.push(serde_json::from_str(&line).map_err(|e| format!("samples.jsonl line {}: {e}", i + 1))?);
    }
    let sampler = fs::read_to_string(dir.join("sampler.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
    Ok((meta, samples, sampler))
}

/// Derive every report file from raw data. `analyze` on an existing run writes
/// into a new sibling directory instead of replacing files.
pub fn write_reports(dir: &Path, meta: RunMeta, samples: &[Sample], sampler: Option<(f64, f64)>) -> Result<Summary, String> {
    let s = summarize(meta, samples, sampler);
    let mut pc = String::from("seq,t_s,pid,create_filetime,role,name,cpu_100ns,private_bytes,working_set_bytes,error\n");
    for x in samples {
        for o in &x.procs {
            let n = |v: Option<u64>| v.map_or(String::new(), |v| v.to_string());
            pc += &format!("{},{:.3},{},{},{:?},{},{},{},{},{}\n", x.seq, x.t_ns as f64 / 1e9, o.id.pid, o.id.create, o.role, o.name.replace(',', "_"), n(o.cpu), n(o.private_bytes), n(o.working_set), o.error.clone().unwrap_or_default().replace(',', ";"));
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
    serde_json::from_slice(&fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?).map_err(|e| format!("{}: {e}", p.display()))
}
