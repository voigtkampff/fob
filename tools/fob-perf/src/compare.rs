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
        "schema={} machine={} cores={} interval={} settle={} steady={} refresh={} helper={} scope={} excluded={:?} settings={:?}",
        m.schema, m.machine, m.logical_cores, m.interval_ms, m.settle_s, m.steady_s, m.refresh_ms, m.helper, m.scope, s.excluded, set
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
    let mut skipped = vec![];
    let mut used: Vec<&Summary> = vec![];
    for r in runs {
        if r.meta.label != label_a && r.meta.label != label_b {
            return Err(format!("run {} has label {:?}, expected {label_a} or {label_b}", r.meta.run_id, r.meta.label));
        }
        if r.quality != "complete" {
            skipped.push(format!("{} ({}): {}", r.meta.run_id, r.quality, r.problems.join("; ")));
        } else {
            used.push(r);
        }
    }
    if let Some(first) = used.first() {
        let k = key(first);
        for r in &used {
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
    let mut confounders = vec![];
    let ver = |l: &str| {
        let mut v: Vec<String> = used.iter().filter(|r| r.meta.label == l).map(|r| format!("{} sha256={}", r.meta.app_version, r.meta.root_exe_sha256.as_deref().unwrap_or("unknown"))).collect();
        v.sort();
        v.dedup();
        v
    };
    let (va, vb) = (ver(label_a), ver(label_b));
    if va.len() > 1 || vb.len() > 1 {
        confounders.push(format!("one side mixes app builds: A {va:?} B {vb:?}"));
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
        rs.sort_by(|a, b| a.meta.started_utc.cmp(&b.meta.started_utc));
        let order: Vec<String> = rs.iter().map(|r| r.meta.label.clone()).collect();
        let switches = order.windows(2).filter(|w| w[0] != w[1]).count();
        let side = |l: &str| rs.iter().filter(|r| r.meta.label == l).copied().collect::<Vec<_>>();
        let (a, b) = (side(label_a), side(label_b));
        let interleaved = !a.is_empty() && !b.is_empty() && switches + 1 >= 2 * a.len().min(b.len());
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
            } else if !interleaved {
                "inconclusive: runs not interleaved".into()
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
