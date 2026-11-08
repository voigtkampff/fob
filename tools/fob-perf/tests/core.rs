mod common;
use common::*;
use fob_perf::analyze::{intervals, summarize};
use fob_perf::model::*;
use fob_perf::tree::find_roots;

fn pids(s: &Sample) -> Vec<u32> {
    s.procs.iter().map(|o| o.id.pid).collect()
}

#[test]
fn scope_is_root_and_owned_webview_only() {
    let mut os = fob();
    let s = run(&mut os, 1, |_, _| {});
    assert_eq!(pids(&s[0]), vec![100, 110, 111]);
    assert!(s[0].excluded.iter().any(|x| x.name == "fob-sensors.exe" && x.reason == "helper"));
    assert!(!pids(&s[0]).contains(&200), "unrelated WebView must not be counted");
}

#[test]
fn roots_ignore_child_fob_and_find_multiple_instances() {
    let mut os = fob();
    os.add(120, 100, "fob.exe", WALL0, 1);
    assert_eq!(find_roots(&os.entries().unwrap(), "fob.exe"), vec![100]);
    os.add(400, 50, "Fob.exe", WALL0, 1);
    assert_eq!(find_roots(&os.entries().unwrap(), "fob.exe"), vec![100, 400]);
    os.kill(100);
    os.kill(120);
    os.kill(400);
    assert!(find_roots(&os.entries().unwrap(), "fob.exe").is_empty());
}

#[test]
fn cpu_is_delta_over_elapsed_over_cores() {
    // 8 cores; root burns 0.4 s and child 0.4 s per 1 s interval = 10 % of machine.
    let mut os = fob();
    let s = run(&mut os, 11, |i, os| if i > 0 { os.burn(100, 4_000_000); os.burn(110, 4_000_000) });
    let iv = intervals(&s, 8, 1000);
    assert_eq!(iv.len(), 10);
    for i in &iv {
        assert!((i.cpu_pct.unwrap() - 10.0).abs() < 1e-9);
    }
    let sum = summarize(meta("A", "tray", 0, 10, 8), &s, None);
    assert_eq!(sum.quality, "complete", "{:?}", sum.problems);
    assert!((sum.cpu_mean_pct.unwrap() - 10.0).abs() < 1e-9);
    // Private bytes summed over root+children only: 120+90+60.
    assert_eq!(sum.private_steady.mean_mb, Some(270.0));
    assert_eq!(sum.working_set_sum_steady.mean_mb, Some(300.0));
    assert_eq!(sum.private_drift_mb_per_h, Some(0.0));
}

#[test]
fn mean_is_time_weighted_and_peak_is_per_interval() {
    let mut os = fob();
    // interval 1: 1 s, 1 core-second; interval 2: 3 s late, 0 cpu. 4 cores.
    let r = root(&os);
    let mut s = vec![];
    let ts = [0u64, 1, 4];
    for (i, t) in ts.iter().enumerate() {
        if i == 1 {
            os.burn(100, 10_000_000);
        }
        let e = os.entries();
        s.push(fob_perf::collect::sample(i as u64, t * 1_000_000_000, WALL0 + t * SEC, r, e, &mut os).0);
    }
    let sum = summarize(meta("A", "tray", 0, 4, 4), &s, None);
    assert!((sum.cpu_mean_pct.unwrap() - 6.25).abs() < 1e-9); // 1 / 4 s / 4 cores
    assert_eq!(sum.cpu_interval_peak_pct, Some(25.0));
    assert_eq!(sum.late_intervals, 1);
    assert_eq!(sum.quality, "qualified");
}

#[test]
fn child_birth_requires_a_baseline_not_wall_clock_inference() {
    let mut os = fob();
    let s = run(&mut os, 3, |i, os| if i == 1 {
        os.add(130, 100, "msedgewebview2.exe", WALL0 + SEC / 2, 30);
        os.burn(130, 2_000_000);
    });
    let iv = intervals(&s, 1, 1000);
    assert_eq!(iv[0].born, 1);
    assert!(iv[0].cpu_pct.is_none());
    assert!(!iv[0].clean());
    assert!(iv[1].clean());
}

#[test]
fn exited_child_marks_interval_and_run_qualified() {
    let mut os = fob();
    let s = run(&mut os, 5, |i, os| if i == 2 { os.kill(111) });
    let iv = intervals(&s, 1, 1000);
    assert_eq!(iv[1].exited, 1);
    assert!(!iv[1].clean());
    let sum = summarize(meta("A", "mini", 0, 4, 1), &s, None);
    assert_eq!(sum.children_exited, 1);
    assert_eq!(sum.quality, "qualified");
    assert_eq!(sum.steady_clean_intervals, 3);
}

#[test]
fn root_exit_and_pid_reuse_invalidate_the_run() {
    let mut os = fob();
    let s = run(&mut os, 3, |i, os| if i == 2 { os.kill(100) });
    assert_eq!(s[2].root, RootState::Exited);
    assert!(intervals(&s, 1, 1000)[1].cpu_pct.is_none());
    assert_eq!(summarize(meta("A", "tray", 0, 2, 1), &s, None).quality, "invalid");

    let mut os = fob();
    let s = run(&mut os, 3, |i, os| if i == 2 { os.kill(100); os.add(100, 50, "fob.exe", WALL0 + 2 * SEC, 50) });
    assert_eq!(s[2].root, RootState::PidReused);
    assert!(summarize(meta("A", "tray", 0, 2, 1), &s, None).problems.iter().any(|p| p == "root pidreused"));
}

#[test]
fn reused_child_pid_with_stale_parent_is_not_counted() {
    // pid 110 exits, its old child 111 survives; a new unrelated process takes pid 110
    // but was created after 111, so 111 (older than its "parent") must be excluded.
    let mut os = fob();
    let s = run(&mut os, 2, |i, os| if i == 1 {
        os.kill(110);
        os.add(110, 100, "msedgewebview2.exe", WALL0 + SEC / 2, 5);
    });
    assert_eq!(pids(&s[1]), vec![100, 110]);
    assert!(s[1].excluded.iter().any(|x| x.pid == 111 && x.reason == "older_than_parent"));
    let iv = intervals(&s, 1, 1000);
    assert_eq!((iv[0].born, iv[0].exited), (1, 2));
}

#[test]
fn access_denied_is_an_error_not_zero() {
    let mut os = fob();
    let s = run(&mut os, 5, |i, os| if i == 1 { os.denied.push(110) } else { os.denied.clear() });
    let bad = s[1].procs.iter().find(|o| o.id.pid == 110).unwrap();
    assert!(bad.error.as_deref().unwrap().contains("Access is denied"));
    assert!(bad.cpu.is_none() && bad.private_bytes.is_none());
    let sum = summarize(meta("A", "tray", 0, 4, 1), &s, None);
    assert!(sum.missing_reads > 0);
    assert_eq!(sum.quality, "qualified");
    assert_eq!(sum.steady_clean_intervals, 2);
    assert_eq!(sum.private_steady.samples, 4, "sample with unreadable child has no private total");
}

#[test]
fn unreadable_memory_and_failed_discovery_are_reported() {
    let mut os = fob();
    let s = run(&mut os, 3, |i, os| {
        os.info.get_mut(&111).unwrap().2 = if i == 1 { None } else { Some(60 << 20) };
        os.list_fails = i == 2;
    });
    assert_eq!(s[1].procs[2].error.as_deref(), Some("memory counters unreadable"));
    assert!(s[2].discovery_error.is_some());
    let sum = summarize(meta("A", "tray", 0, 2, 1), &s, None);
    assert_eq!(sum.discovery_errors, 1);
    assert_eq!(sum.quality, "invalid");
}

#[test]
fn settle_window_is_reported_separately_and_drift_measured() {
    let mut os = fob();
    // Startup spike at 1000 MB in the first 3 s, then a steady 1 MB/s growth.
    let s = run(&mut os, 10, |i, os| {
        let mb = if i < 3 { 1000 } else { 100 + i };
        os.info.get_mut(&100).unwrap().2 = Some(mb << 20);
    });
    let sum = summarize(meta("A", "tray", 3, 6, 1), &s, None);
    assert_eq!(sum.private_settle.max_mb, Some(1150.0));
    assert_eq!(sum.private_steady.max_mb, Some(259.0));
    assert!((sum.private_drift_mb_per_h.unwrap() - 3600.0).abs() < 1e-6);
    assert_eq!(sum.steady_intervals, 6);
}

#[test]
fn short_run_is_qualified() {
    let mut os = fob();
    let s = run(&mut os, 4, |_, _| {});
    let sum = summarize(meta("A", "tray", 0, 60, 1), &s, None);
    assert!(sum.problems.iter().any(|p| p.contains("shorter than requested")));
    assert_eq!(sum.quality, "qualified");
}

#[test]
fn many_processor_groups_normalise_by_all_cores() {
    let mut os = fob();
    let s = run(&mut os, 2, |i, os| if i == 1 { os.burn(100, 128 * SEC) });
    assert!((intervals(&s, 128, 1000)[0].cpu_pct.unwrap() - 100.0).abs() < 1e-9);
}

#[test]
fn arbitrary_descendants_are_not_webviews() {
    let mut os = fob();
    os.add(150, 100, "notepad.exe", WALL0, 5);
    os.add(151, 150, "msedgewebview2.exe", WALL0, 5);
    let s = run(&mut os, 1, |_, _| {});
    assert_eq!(pids(&s[0]), vec![100, 110, 111]);
}

#[test]
fn losing_one_requested_interval_is_not_complete() {
    let mut os = fob();
    let s = run(&mut os, 10, |_, _| {});
    assert_ne!(summarize(meta("A", "tray", 0, 10, 8), &s, None).quality, "complete");
}

#[test]
fn a_sequence_gap_cannot_be_complete() {
    let mut os = fob();
    let mut s = run(&mut os, 11, |_, _| {});
    for x in s.iter_mut().skip(5) { x.seq += 1; }
    assert_ne!(summarize(meta("A", "tray", 0, 10, 8), &s, None).quality, "complete");
}

#[test]
fn cpu_uses_counter_read_times_not_discovery_start() {
    let mut os = fob();
    let mut s = run(&mut os, 3, |i, os| if i > 0 { os.burn(100, SEC) });
    s[0].procs[0].cpu_t_ns = 100_000_000;
    s[1].procs[0].cpu_t_ns = 1_300_000_000;
    s[2].procs[0].cpu_t_ns = 2_100_000_000;
    let iv = intervals(&s, 2, 1000);
    assert!((iv[0].cpu_pct.unwrap() - 100.0 / 2.4).abs() < 1e-9);
    assert!((iv[1].cpu_pct.unwrap() - 62.5).abs() < 1e-9);
    assert_ne!(summarize(meta("A", "tray", 0, 2, 2), &s, None).quality, "complete");
}

#[test]
fn smoke_rejects_empty_failed_discovery_and_lost_root() {
    let mut os = fob();
    let s = run(&mut os, 3, |i, os| { if i == 1 { os.list_fails = true; } if i == 2 { os.list_fails = false; os.kill(100); } });
    assert!(fob_perf::collect::smoke_ready(&s[0]));
    assert!(!fob_perf::collect::smoke_ready(&s[1]));
    assert!(!fob_perf::collect::smoke_ready(&s[2]));
}

#[test]
fn missing_working_set_and_initial_memory_are_not_complete() {
    let mut os = fob();
    let mut s = run(&mut os, 11, |_, _| {});
    s[0].procs[0].working_set = None;
    let r = summarize(meta("A", "tray", 2, 8, 8), &s, None);
    assert_ne!(r.quality, "complete");
    assert_eq!(r.missing_reads, 1);
}

#[test]
fn changed_or_unavailable_executable_hash_disqualifies() {
    let mut os = fob();
    let s = run(&mut os, 11, |_, _| {});
    let mut m = meta("A", "tray", 0, 10, 8);
    m.root_exe_sha256 = None;
    m.root_exe_bytes = None;
    assert_ne!(summarize(m, &s, None).quality, "complete");
}

#[test]
fn backwards_wall_clock_does_not_invent_birth_cpu() {
    let mut os = fob();
    let mut s = run(&mut os, 3, |i, os| if i == 1 { os.add(130, 100, "msedgewebview2.exe", WALL0 - SEC, 2); os.burn(130, SEC); });
    s[0].wall -= 1000 * SEC;
    let iv = intervals(&s, 8, 1000);
    assert!(iv[0].cpu_pct.is_none());
    assert!(iv[1].clean());
}

#[test]
fn failed_root_metrics_do_not_pass_smoke() {
    let mut os = fob();
    let mut s = run(&mut os, 1, |_, _| {});
    s[0].procs[0].cpu = None;
    assert!(!fob_perf::collect::smoke_ready(&s[0]));
}

#[test]
fn too_few_memory_points_cannot_claim_complete() {
    let mut os = fob();
    let s = run(&mut os, 2, |_, _| {});
    let r = summarize(meta("A", "tray", 0, 1, 8), &s, None);
    assert_ne!(r.quality, "complete");
    assert!(r.private_drift_mb_per_h.is_none());
}
