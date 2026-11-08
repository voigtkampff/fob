//! Turns raw samples into interval CPU, memory series and a run summary.
use crate::model::{Identity, Obs, RootState, RunMeta, Sample};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    pub seq: u64,
    pub start_s: f64,
    pub elapsed_s: f64,
    /// CPU % of all logical cores, None for incomplete counter intervals.
    pub cpu_pct: Option<f64>,
    pub processes: usize,
    pub born: usize,
    pub exited: usize,
    pub missing: usize,
    pub late: bool,
}

impl Interval {
    pub fn clean(&self) -> bool {
        self.cpu_pct.is_some() && self.exited == 0 && self.missing == 0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MemStats {
    pub samples: usize,
    pub mean_mb: Option<f64>,
    pub min_mb: Option<f64>,
    pub max_mb: Option<f64>,
    pub first_mb: Option<f64>,
    pub last_mb: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcStats {
    pub pid: u32,
    pub create: u64,
    pub name: String,
    pub role: String,
    pub samples: usize,
    pub cpu_s_steady: f64,
    pub private_max_mb: Option<f64>,
    pub working_set_max_mb: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub meta: RunMeta,
    /// complete | qualified | invalid
    pub quality: String,
    pub problems: Vec<String>,
    pub samples: usize,
    pub steady_intervals: usize,
    pub steady_clean_intervals: usize,
    pub steady_elapsed_s: f64,
    pub late_intervals: usize,
    pub children_born: usize,
    pub children_exited: usize,
    pub missing_reads: usize,
    pub discovery_errors: usize,
    pub excluded: Vec<String>,
    /// Time-weighted mean over steady intervals, % of all logical cores.
    pub cpu_mean_pct: Option<f64>,
    /// Highest single sampling-interval value, not an instantaneous maximum.
    pub cpu_interval_peak_pct: Option<f64>,
    pub cpu_interval_p95_pct: Option<f64>,
    pub private_settle: MemStats,
    pub private_steady: MemStats,
    pub working_set_sum_steady: MemStats,
    /// Least-squares slope of total private bytes over the steady phase.
    pub private_drift_mb_per_h: Option<f64>,
    pub processes: Vec<ProcStats>,
    pub sampler_cpu_s: Option<f64>,
    pub sampler_peak_ws_mb: Option<f64>,
}

const MB: f64 = 1024.0 * 1024.0;

pub fn intervals(samples: &[Sample], cores: u32, interval_ms: u64) -> Vec<Interval> {
    let mut out = vec![];
    for w in samples.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let elapsed_s = (b.t_ns.saturating_sub(a.t_ns)) as f64 / 1e9;
        let prev: HashMap<Identity, &Obs> = a.procs.iter().map(|o| (o.id, o)).collect();
        let cur: HashMap<Identity, &Obs> = b.procs.iter().map(|o| (o.id, o)).collect();
        let mut cpu_rate = 0.0;
        let (mut born, mut missing) = (0, 0);
        for o in &b.procs {
            if !prev.contains_key(&o.id) && o.role == crate::model::Role::Child { born += 1; }
            match (prev.get(&o.id), o.cpu) {
                (Some(p), Some(c)) if p.cpu.is_some_and(|v| c >= v) && o.cpu_t_ns > p.cpu_t_ns => {
                    cpu_rate += (c - p.cpu.unwrap()) as f64 / 1e7
                        / ((o.cpu_t_ns - p.cpu_t_ns) as f64 / 1e9);
                }
                _ => missing += 1,
            }
        }
        missing += a.discovery_error.is_some() as usize + b.discovery_error.is_some() as usize;
        missing += (b.seq != a.seq + 1) as usize;
        let exited = a.procs.iter().filter(|o| o.role == crate::model::Role::Child && !cur.contains_key(&o.id)).count();
        let ok = a.root == RootState::Alive && b.root == RootState::Alive && elapsed_s > 0.0
            && cores > 0 && !a.procs.is_empty() && !b.procs.is_empty() && missing == 0 && exited == 0;
        out.push(Interval {
            seq: b.seq,
            start_s: a.t_ns as f64 / 1e9,
            elapsed_s,
            cpu_pct: ok.then(|| cpu_rate / cores as f64 * 100.0),
            processes: b.procs.len(),
            born,
            exited,
            missing,
            late: elapsed_s * 1000.0 > interval_ms as f64 * 1.5,
        });
    }
    out
}

fn sum_mem(s: &Sample, f: impl Fn(&Obs) -> Option<u64>) -> Option<f64> {
    if s.root != RootState::Alive || s.discovery_error.is_some() || s.procs.is_empty() {
        return None;
    }
    s.procs.iter().try_fold(0u64, |sum, o| sum.checked_add(f(o)?)).map(|b| b as f64 / MB)
}

fn mem_stats(v: &[f64]) -> MemStats {
    if v.is_empty() {
        return MemStats::default();
    }
    MemStats {
        samples: v.len(),
        mean_mb: Some(v.iter().sum::<f64>() / v.len() as f64),
        min_mb: v.iter().cloned().reduce(f64::min),
        max_mb: v.iter().cloned().reduce(f64::max),
        first_mb: v.first().copied(),
        last_mb: v.last().copied(),
    }
}

fn slope(points: &[(f64, f64)]) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    (sxx > 0.0).then(|| sxy / sxx * 3600.0)
}

pub fn summarize(meta: RunMeta, samples: &[Sample], sampler: Option<(f64, f64)>) -> Summary {
    let settle = meta.settle_s as f64;
    let ivs = intervals(samples, meta.logical_cores, meta.interval_ms);
    let end = settle + meta.steady_s as f64;
    let steady: Vec<&Interval> = ivs.iter().filter(|i| i.start_s >= settle && i.start_s < end).collect();
    let mut problems = vec![];
    if samples.first().is_none_or(|s| s.seq != 0 || s.t_ns > 20_000_000) {
        problems.push("missing initial sample".into());
    }
    let failed_reads = samples.iter().flat_map(|s| &s.procs)
        .filter(|o| o.error.is_some() || o.cpu.is_none() || o.private_bytes.is_none() || o.working_set.is_none()).count();
    if failed_reads > 0 { problems.push(format!("{failed_reads} process samples with incomplete metrics")); }
    if ivs.iter().any(|i| !i.clean()) {
        problems.push("incomplete CPU interval in run, including settle".into());
    }
    if samples.iter().any(|s| s.procs.iter().any(|o| o.cpu_t_ns < s.t_ns
        || o.cpu_t_ns.saturating_sub(s.t_ns) > meta.interval_ms * 1_000_000 / 20)) {
        problems.push("counter acquisition skew exceeds 5% of sampling interval".into());
    }
    if samples.last().is_some_and(|s| s.t_ns as f64 / 1e9 > end + 0.02) {
        problems.push("sampling exceeded requested phase boundary".into());
    }
    if meta.root_exe_sha256.is_none() || meta.root_exe_bytes.is_none() {
        problems.push("executable metadata unavailable or changed during run".into());
    }


    let mut root_lost = None;
    for s in samples {
        if s.root != RootState::Alive {
            root_lost = Some(s.root);
            break;
        }
    }
    if let Some(r) = root_lost {
        problems.push(format!("root {:?}", r).to_lowercase());
    }
    let clean = steady.iter().filter(|i| i.clean()).count();
    let unclean = steady.len() - clean;
    if unclean > 0 {
        problems.push(format!("{unclean} steady intervals missing data or losing an exited child"));
    }
    let steady_elapsed: f64 = steady.iter().map(|i| i.elapsed_s).sum();
    if steady_elapsed + 0.02 < meta.steady_s as f64 {
        problems.push(format!("steady phase {:.0}s shorter than requested {}s", steady_elapsed, meta.steady_s));
    }
    let late = steady.iter().filter(|i| i.late).count();
    if late > 0 {
        problems.push(format!("{late} late sampling intervals"));
    }

    // CPU over clean steady intervals only, weighted by real elapsed time.
    let good: Vec<&&Interval> = steady.iter().filter(|i| i.clean()).collect();
    let el: f64 = good.iter().map(|i| i.elapsed_s).sum();
    let cpu_mean = (el > 0.0).then(|| good.iter().map(|i| i.cpu_pct.unwrap() * i.elapsed_s).sum::<f64>() / el);
    let mut pcts: Vec<f64> = good.iter().map(|i| i.cpu_pct.unwrap()).collect();
    pcts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = (!pcts.is_empty()).then(|| pcts[((pcts.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)]);

    let (mut settle_priv, mut steady_priv, mut steady_ws, mut pts) = (vec![], vec![], vec![], vec![]);
    let mut mem_missing = 0;
    let mut ws_missing = 0;
    for s in samples {
        let t = s.t_ns as f64 / 1e9;
        if t > end + 0.02 { continue; }
        let p = sum_mem(s, |o| o.private_bytes);
        if t < settle {
            settle_priv.extend(p);
            continue;
        }
        match p {
            Some(v) => {
                steady_priv.push(v);
                pts.push((t, v));
            }
            None => mem_missing += 1,
        }
        match sum_mem(s, |o| o.working_set) {
            Some(v) => steady_ws.push(v),
            None => ws_missing += 1,
        }
    }
    if pts.len() < 3 { problems.push("insufficient steady memory points for trend".into()); }
    if mem_missing > 0 {
        problems.push(format!("{mem_missing} steady samples without complete private bytes"));
    }

    if ws_missing > 0 {
        problems.push(format!("{ws_missing} steady samples without complete working set"));
    }

    let mut per: BTreeMap<Identity, ProcStats> = BTreeMap::new();
    for s in samples {
        for o in &s.procs {
            let e = per.entry(o.id).or_insert_with(|| ProcStats {
                pid: o.id.pid,
                create: o.id.create,
                name: o.name.clone(),
                role: format!("{:?}", o.role).to_lowercase(),
                samples: 0,
                cpu_s_steady: 0.0,
                private_max_mb: None,
                working_set_max_mb: None,
            });
            e.samples += 1;
            let mx = |cur: Option<f64>, v: Option<u64>| match (cur, v) {
                (c, Some(b)) => Some(c.map_or(b as f64 / MB, |c| c.max(b as f64 / MB))),
                (c, None) => c,
            };
            e.private_max_mb = mx(e.private_max_mb, o.private_bytes);
            e.working_set_max_mb = mx(e.working_set_max_mb, o.working_set);
        }
    }
    for w in samples.windows(2) {
        if (w[0].t_ns as f64 / 1e9) < settle {
            continue;
        }
        let prev: HashMap<Identity, Option<u64>> = w[0].procs.iter().map(|o| (o.id, o.cpu)).collect();
        for o in &w[1].procs {
            if let (Some(Some(p)), Some(c)) = (prev.get(&o.id), o.cpu) {
                if c >= *p {
                    per.get_mut(&o.id).unwrap().cpu_s_steady += (c - p) as f64 / 1e7;
                }
            }
        }
    }

    let mut excluded: Vec<String> = samples
        .iter()
        .flat_map(|s| s.excluded.iter().map(|x| format!("{} ({})", x.name, x.reason)))
        .collect();
    excluded.sort();
    excluded.dedup();

    let quality = if root_lost.is_some() || cpu_mean.is_none() || steady_priv.is_empty() {
        "invalid"
    } else if problems.is_empty() {
        "complete"
    } else {
        "qualified"
    };
    Summary {
        quality: quality.into(),
        problems,
        samples: samples.len(),
        steady_intervals: steady.len(),
        steady_clean_intervals: clean,
        steady_elapsed_s: steady_elapsed,
        late_intervals: late,
        children_born: ivs.iter().map(|i| i.born).sum(),
        children_exited: ivs.iter().map(|i| i.exited).sum(),
        missing_reads: failed_reads,
        discovery_errors: samples.iter().filter(|s| s.discovery_error.is_some()).count(),
        excluded,
        cpu_mean_pct: cpu_mean,
        cpu_interval_peak_pct: pcts.last().copied(),
        cpu_interval_p95_pct: p95,
        private_settle: mem_stats(&settle_priv),
        private_steady: mem_stats(&steady_priv),
        working_set_sum_steady: mem_stats(&steady_ws),
        private_drift_mb_per_h: slope(&pts),
        processes: per.into_values().collect(),
        sampler_cpu_s: sampler.map(|s| s.0),
        sampler_peak_ws_mb: sampler.map(|s| s.1),
        meta,
    }
}

fn f(v: Option<f64>, d: usize) -> String {
    v.map_or("无数据".into(), |v| format!("{v:.d$}"))
}

pub fn render(s: &Summary) -> String {
    let m = &s.meta;
    let mut o = String::new();
    o += &format!("运行 {}  标签 {}  声明版本 {}  声明状态 {}\n", m.run_id, m.label, m.app_version, m.mode);
    o += &format!("结果质量：{}\n", match s.quality.as_str() { "complete" => "完整", "qualified" => "有缺失，仅供参考", _ => "无效" });
    for p in &s.problems {
        o += &format!("  问题：{p}\n");
    }
    o += &format!("声明刷新间隔 {} ms，声明助手状态 {}，设置 {:?}（未自动读取）\n", m.refresh_ms, m.helper, m.settings);
    o += &format!("实测可执行文件 {}，SHA256 {}\n", m.root_exe_name, m.root_exe_sha256.as_deref().unwrap_or("不可确认"));
    o += &format!("范围：{}\n", m.scope);
    o += &format!("排除：{}\n", if s.excluded.is_empty() { "无".into() } else { s.excluded.join(", ") });
    o += &format!("逻辑处理器 {}（{} 组），采样间隔 {} ms，稳定前等待 {} s，稳定段 {:.0} s\n", m.logical_cores, m.processor_groups, m.interval_ms, m.settle_s, s.steady_elapsed_s);
    o += &format!("CPU 平均（占全部逻辑处理器）：{} %\n", f(s.cpu_mean_pct, 3));
    o += &format!("CPU 单个采样间隔最高：{} %，P95：{} %\n", f(s.cpu_interval_peak_pct, 2), f(s.cpu_interval_p95_pct, 2));
    o += &format!("私有提交内存合计（稳定段）：平均 {} MB，最低 {} MB，最高 {} MB\n", f(s.private_steady.mean_mb, 1), f(s.private_steady.min_mb, 1), f(s.private_steady.max_mb, 1));
    o += &format!("私有提交内存变化趋势：{} MB/小时（不能据此判定泄漏）\n", f(s.private_drift_mb_per_h, 1));
    o += &format!("附加后等待阶段私有内存最高：{} MB（附加时 Fob 已运行 {:.0} s）\n", f(s.private_settle.max_mb, 1), m.root_age_at_attach_s);
    o += &format!("工作集相加（含共享页，可能重复计算）：平均 {} MB\n", f(s.working_set_sum_steady.mean_mb, 1));
    o += &format!("子进程新增 {}，退出 {}，读取缺失 {}，发现失败 {}\n", s.children_born, s.children_exited, s.missing_reads, s.discovery_errors);
    o += &format!("采样工具自身 CPU {} s，峰值工作集 {} MB（不计入 Fob，含附加与哈希，到报告生成前）\n", f(s.sampler_cpu_s, 2), f(s.sampler_peak_ws_mb, 1));
    o
}
