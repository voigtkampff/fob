mod common;
use common::*;
use fob_perf::analyze::{summarize, Summary};
use fob_perf::compare::compare;

/// One steady run where root burns `cpu_ms` per second.
fn runrec(label: &str, mode: &str, order: u32, cpu_ms: u64) -> Summary {
    let mut os = fob();
    let s = run(&mut os, 11, |i, os| if i > 0 { os.burn(100, cpu_ms * 10_000) });
    let mut m = meta(label, mode, 0, 10, 8);
    m.run_id = format!("{label}{order}-{mode}");
    m.started_utc = format!("unix:{:012}", order as u64 * 20);
    if label == "B" {
        m.app_version = "0.8.2".into();
        m.root_exe_sha256 = Some("bb".repeat(32));
    } else {
        m.app_version = "0.8.1".into();
    }
    summarize(m, &s, None)
}

fn interleaved(a: &[u64], b: &[u64]) -> Vec<Summary> {
    let mut v = vec![];
    for i in 0..a.len().max(b.len()) {
        if let Some(c) = a.get(i) { v.push(runrec("A", "tray", (2 * i) as u32, *c)); }
        if let Some(c) = b.get(i) { v.push(runrec("B", "tray", (2 * i + 1) as u32, *c)); }
    }
    v
}

#[test]
fn clear_separation_with_five_interleaved_runs() {
    let r = compare(&interleaved(&[80, 82, 81, 79, 80], &[40, 41, 39, 40, 42]), "A", "B").unwrap();
    let m = &r.modes[0].metrics[0];
    assert_eq!(m.metric, "cpu_mean_pct");
    assert_eq!(m.verdict, "B lower: ranges do not overlap");
    assert!((m.diff_b_minus_a - (-0.5)).abs() < 0.01);
    assert!(r.confounders.iter().any(|c| c.contains("different builds")));
}

#[test]
fn identical_or_noisy_is_inconclusive() {
    let r = compare(&interleaved(&[80; 5], &[80; 5]), "A", "B").unwrap();
    assert!(r.modes[0].metrics[0].verdict.contains("within run-to-run"));
    let r = compare(&interleaved(&[80, 20, 90, 30, 60], &[50, 70, 40, 85, 25]), "A", "B").unwrap();
    assert!(r.modes[0].metrics[0].verdict.starts_with("inconclusive"));
}

#[test]
fn too_few_runs_is_inconclusive() {
    let r = compare(&interleaved(&[80, 80, 80], &[10, 10, 10]), "A", "B").unwrap();
    assert!(r.modes[0].metrics[0].verdict.contains(">= 5"));
}

#[test]
fn blocked_order_is_flagged() {
    let mut v: Vec<Summary> = (0..5).map(|i| runrec("A", "tray", i, 80)).collect();
    v.extend((5..10).map(|i| runrec("B", "tray", i, 10)));
    let r = compare(&v, "A", "B").unwrap();
    assert!(!r.modes[0].interleaved);
    assert_eq!(r.modes[0].order.join(""), "AAAAABBBBB");
    assert!(r.modes[0].metrics[0].verdict.contains("not interleaved"));
}

#[test]
fn incompatible_runs_are_rejected() {
    let mut v = interleaved(&[80; 2], &[80; 2]);
    v[1].meta.refresh_ms = 1000;
    assert!(compare(&v, "A", "B").unwrap_err().contains("incompatible"));
    let mut v = interleaved(&[80; 2], &[80; 2]);
    v[1].meta.machine = "other".into();
    assert!(compare(&v, "A", "B").is_err());
    let mut v = interleaved(&[80; 2], &[80; 2]);
    v[3].meta.label = "C".into();
    assert!(compare(&v, "A", "B").is_err());
    let v = vec![runrec("A", "tray", 0, 1), runrec("A", "tray", 0, 1)];
    assert!(compare(&v, "A", "B").unwrap_err().contains("twice"));
}

#[test]
fn incomplete_runs_are_skipped_not_compared() {
    let mut v = interleaved(&[80; 5], &[10; 5]);
    v[0].quality = "qualified".into();
    v[0].problems = vec!["1 late sampling intervals".into()];
    let r = compare(&v, "A", "B").unwrap();
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.modes[0].a_runs, 4);
    assert!(r.modes[0].metrics[0].verdict.contains(">= 5"));
}

#[test]
fn modes_are_compared_separately() {
    let mut v = interleaved(&[80; 5], &[10; 5]);
    v.push(runrec("A", "mini", 20, 5));
    let r = compare(&v, "A", "B").unwrap();
    let mini = r.modes.iter().find(|m| m.mode == "mini").unwrap();
    assert_eq!((mini.a_runs, mini.b_runs), (1, 0));
    assert!(mini.metrics.is_empty());
}

#[test]
fn mixed_builds_within_one_label_are_rejected() {
    let mut v = interleaved(&[80; 5], &[10; 5]);
    v[2].meta.root_exe_sha256 = Some("cc".repeat(32));
    assert!(compare(&v, "A", "B").is_err());
}

#[test]
fn equal_labels_are_rejected() {
    let v = interleaved(&[80; 1], &[10; 1]);
    assert!(compare(&v[..1], "A", "A").is_err());
}

#[test]
fn timestamps_must_be_nonoverlapping_and_strictly_alternating() {
    let mut v = interleaved(&[80; 5], &[10; 5]);
    v[1].meta.started_utc = v[0].meta.started_utc.clone();
    let r = compare(&v, "A", "B").unwrap();
    assert!(!r.modes[0].interleaved);
    assert!(r.modes[0].metrics[0].verdict.starts_with("inconclusive"));
    let mut v = interleaved(&[80; 6], &[10; 5]);
    v[10].meta.started_utc = "unix:000000000001".into();
    assert!(!compare(&v, "A", "B").unwrap().modes[0].interleaved);
}

#[test]
fn invalid_metrics_and_root_metadata_are_rejected() {
    let mut v = interleaved(&[80; 5], &[10; 5]);
    v[0].cpu_interval_p95_pct = None;
    assert!(compare(&v, "A", "B").is_err());
    let mut v = interleaved(&[80; 5], &[10; 5]);
    v[0].meta.root.pid = 42;
    assert!(compare(&v, "A", "B").is_err());
    let mut v = interleaved(&[80; 5], &[10; 5]);
    v[0].meta.processor_groups = 2;
    assert!(compare(&v, "A", "B").is_err());
}
