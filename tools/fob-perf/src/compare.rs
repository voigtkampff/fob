//! A/B comparison of complete runs. Picks no winner unless the data supports it.
use crate::analyze::Summary;
use serde::Serialize;
use std::collections::BTreeMap;

pub const MIN_RUNS: usize = 5;

#[derive(Debug, Serialize)]
pub struct Metric {
    pub metric: String,
    pub a_mean: f64,
    pub a_sd: f64,
    pub a_min: f64,
    pub a_max: f64,
    pub b_mean: f64,
    pub b_sd: f64,
    pub b_min: f64,
    pub b_max: f64,
    pub diff_b_minus_a: f64,
    pub verdict: String,
}

#[derive(Debug, Serialize)]
pub struct ModeResult {
    pub mode: String,
    pub a_runs: usize,
    pub b_runs: usize,
    pub order: Vec<String>,
    pub interleaved: bool,
    pub metrics: Vec<Metric>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub label_a: String,
    pub label_b: String,
    pub skipped: Vec<String>,
    pub confounders: Vec<String>,
    pub modes: Vec<ModeResult>,
}

fn key(s: &Summary) -> String {
    let m = &s.meta;
    let mut set = m.settings.clone();
    set.sort();
    format!(
        "schema={} tool={} machine={} cores={} groups={} interval={} settle={} steady={} refresh={} helper={} scope={} excluded={:?} settings={:?}",
        m.schema, m.tool_version, m.machine, m.logical_cores, m.processor_groups, m.interval_ms, m.settle_s, m.steady_s, m.refresh_ms, m.helper, m.scope, s.excluded, set
    )
}

fn stats(v: &[f64]) -> (f64, f64, f64, f64) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let sd = if v.len() > 1 { (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt() } else { 0.0 };
    (mean, sd, v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
}

/// Returns Err for incompatible inputs.
pub fn compare(runs: &[Summary], label_a: &str, label_b: &str) -> Result<Report, String> {
    if label_a.is_empty() || label_b.is_empty() || label_a == label_b {
        return Err("comparison needs two distinct labels".into());
    }
    let mut skipped = vec![];
    let mut used: Vec<&Summary> = vec![];
    for r in runs {
        crate::store::validate_meta(&r.meta)?;
        if r.quality == "complete" && (!r.problems.is_empty()
            || [r.cpu_mean_pct, r.cpu_interval_p95_pct, r.private_steady.mean_mb, r.private_drift_mb_per_h]
                .iter().any(|x| x.is_none_or(|v| !v.is_finite()))
            || r.steady_clean_intervals != r.steady_intervals
            || r.steady_elapsed_s + 0.02 < r.meta.steady_s as f64
            || r.meta.root_exe_sha256.is_none()
            || !r.processes.iter().any(|p| p.role == "root" && p.pid == r.meta.root.pid
                && p.create == r.meta.root.create && p.name.eq_ignore_ascii_case(&r.meta.root_exe_name))) {
            return Err(format!("run {} claims complete but lacks consistent metrics/root metadata", r.meta.run_id));
        }
        if r.meta.label != label_a && r.meta.label != label_b {
            return Err(format!("run {} has label {:?}, expected {label_a} or {label_b}", r.meta.run_id, r.meta.label));
        }
        if r.quality != "complete" {
            skipped.push(format!("{} ({}): {}", r.meta.run_id, r.quality, r.problems.join("; ")));
        } else {
            used.push(r);
        }
    }
    if let Some(first) = runs.first() {
        let k = key(first);
        for r in runs {
            if key(r) != k {
                return Err(format!("incompatible runs {} and {}:\n  {}\n  {}", first.meta.run_id, r.meta.run_id, k, key(r)));
            }
        }
    }
    let mut ids = std::collections::HashSet::new();
    for r in runs {
        if !ids.insert(&r.meta.run_id) {
            return Err(format!("run {} given twice", r.meta.run_id));
        }
    }
    let mut confounders = vec!["Directional differences are not proof of lightweight benefit; baseline equivalence and workload were not verified. Memory slope is not proof of a leak.".into()];
    let ver = |l: &str| {
        let mut v: Vec<String> = runs.iter().filter(|r| r.meta.label == l).map(|r| format!("{} sha256={} bytes={:?}", r.meta.app_version, r.meta.root_exe_sha256.as_deref().unwrap_or("unknown"), r.meta.root_exe_bytes)).collect();
        v.sort();
        v.dedup();
        v
    };
    let (va, vb) = (ver(label_a), ver(label_b));
    if va.len() > 1 || vb.len() > 1 {
        return Err(format!("one side mixes app builds: A {va:?} B {vb:?}"));
    }
    if va != vb {
        confounders.push("A and B are different builds; any difference covers every change between them, not one optimisation".into());
    }

    let mut by_mode: BTreeMap<String, Vec<&Summary>> = BTreeMap::new();
    for r in &used {
        by_mode.entry(r.meta.mode.clone()).or_default().push(r);
    }
    let mut modes = vec![];
    for (mode, mut rs) in by_mode {
        let timestamp = |r: &&Summary| r.meta.started_utc.strip_prefix("unix:").unwrap().parse::<u64>().unwrap();
        rs.sort_by_key(timestamp);
        let chronological = rs.windows(2).all(|w| timestamp(&w[0])
            .checked_add(w[0].meta.settle_s + w[0].meta.steady_s)
            .is_some_and(|end| end <= timestamp(&w[1])));
        let has_incomplete = runs.iter().any(|r| r.meta.mode == mode && r.quality != "complete");
        let order: Vec<String> = rs.iter().map(|r| r.meta.label.clone()).collect();
        let side = |l: &str| rs.iter().filter(|r| r.meta.label == l).copied().collect::<Vec<_>>();
        let (a, b) = (side(label_a), side(label_b));
        let interleaved = !a.is_empty() && !b.is_empty() && chronological && order.windows(2).all(|w| w[0] != w[1]);
        let mut metrics = vec![];
        type Get = fn(&Summary) -> Option<f64>;
        let list: [(&str, Get); 4] = [
            ("cpu_mean_pct", |s| s.cpu_mean_pct),
            ("cpu_interval_p95_pct", |s| s.cpu_interval_p95_pct),
            ("private_steady_mean_mb", |s| s.private_steady.mean_mb),
            ("private_drift_mb_per_h", |s| s.private_drift_mb_per_h),
        ];
        for (name, get) in list {
            let av: Vec<f64> = a.iter().filter_map(|s| get(s)).collect();
            let bv: Vec<f64> = b.iter().filter_map(|s| get(s)).collect();
            if av.is_empty() || bv.is_empty() {
                continue;
            }
            let (am, asd, amin, amax) = stats(&av);
            let (bm, bsd, bmin, bmax) = stats(&bv);
            let verdict = if av.len() < MIN_RUNS || bv.len() < MIN_RUNS {
                format!("inconclusive: need >= {MIN_RUNS} complete runs per side")
            } else if has_incomplete {
                "inconclusive: input mode contains incomplete runs".into()
            } else if !interleaved {
                "inconclusive: runs not interleaved or overlap in time".into()
            } else if bmax < amin {
                format!("{label_b} lower: ranges do not overlap")
            } else if bmin > amax {
                format!("{label_b} higher: ranges do not overlap")
            } else {
                "inconclusive: difference within run-to-run variation".into()
            };
            metrics.push(Metric { metric: name.into(), a_mean: am, a_sd: asd, a_min: amin, a_max: amax, b_mean: bm, b_sd: bsd, b_min: bmin, b_max: bmax, diff_b_minus_a: bm - am, verdict });
        }
        modes.push(ModeResult { mode, a_runs: a.len(), b_runs: b.len(), order, interleaved, metrics });
    }
    Ok(Report { label_a: label_a.into(), label_b: label_b.into(), skipped, confounders, modes })
}

pub fn render(r: &Report) -> String {
    let mut o = format!("对比 A={}  B={}\n", r.label_a, r.label_b);
    for c in &r.confounders {
        o += &format!("干扰因素：{c}\n");
    }
    for s in &r.skipped {
        o += &format!("未计入：{s}\n");
    }
    for m in &r.modes {
        o += &format!("\n[{}] A {} 次，B {} 次，顺序 {}{}\n", m.mode, m.a_runs, m.b_runs, m.order.join(""), if m.interleaved { "" } else { "（未交替）" });
        for x in &m.metrics {
            o += &format!(
                "  {}: A {:.3}±{:.3} [{:.3}–{:.3}]  B {:.3}±{:.3} [{:.3}–{:.3}]  B−A {:+.3}  {}\n",
                x.metric, x.a_mean, x.a_sd, x.a_min, x.a_max, x.b_mean, x.b_sd, x.b_min, x.b_max, x.diff_b_minus_a, x.verdict
            );
        }
    }
    o
}
