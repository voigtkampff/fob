mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_fob-perf")).args(args).output().unwrap();
    (o.status.code().unwrap(), String::from_utf8_lossy(&o.stdout).into(), String::from_utf8_lossy(&o.stderr).into())
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fob-perf-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Write a raw run the way `sample` does: run.json + samples.jsonl.
fn raw_run(dir: &Path, label: &str, order: u32, cpu_ms: u64, refresh: u32) {
    std::fs::create_dir(dir).unwrap();
    let mut os = fob();
    let s = run(&mut os, 11, |i, os| if i > 0 { os.burn(100, cpu_ms * 10_000); os.burn(110, 1000) });
    let mut m = meta(label, "processes", 0, 10, 8);
    m.run_id = format!("{label}{order}");
    m.started_utc = format!("unix:{order:012}");
    m.refresh_ms = refresh;
    std::fs::write(dir.join("run.json"), serde_json::to_string(&m).unwrap()).unwrap();
    let lines: Vec<String> = s.iter().map(|x| serde_json::to_string(x).unwrap()).collect();
    std::fs::write(dir.join("samples.jsonl"), lines.join("\n")).unwrap();
}

#[test]
fn analyze_writes_all_reports_to_a_new_directory_only() {
    let d = tmp("analyze");
    raw_run(&d.join("raw"), "A", 0, 80, 2000);
    let out = d.join("out");
    let (code, stdout, err) = bin(&["analyze", d.join("raw").to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(stdout.contains("结果质量：完整"), "{stdout}");
    assert!(stdout.contains("CPU 平均（占全部逻辑处理器）：1.001 %"), "{stdout}");
    for f in ["run.json", "samples.jsonl", "processes.csv", "intervals.csv", "summary.json", "summary.txt"] {
        assert!(out.join(f).exists(), "{f}");
    }
    let csv = std::fs::read_to_string(out.join("processes.csv")).unwrap();
    assert!(csv.lines().all(|l| !l.contains(",200,")), "unrelated pid exported");
    assert_eq!(csv.lines().count(), 1 + 11 * 3);
    let (code, _, err) = bin(&["analyze", d.join("raw").to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(err.contains("cannot create new result directory"), "{err}");
}

#[test]
fn compare_cli_reports_and_rejects() {
    let d = tmp("compare");
    let mut dirs = vec![];
    for i in 0..5u32 {
        for (k, label, cpu) in [(0, "A", 80), (1, "B", 40)] {
            let raw = d.join(format!("raw{label}{i}"));
            raw_run(&raw, label, 2 * i + k, cpu + i as u64, 2000);
            let out = d.join(format!("{label}{i}"));
            assert_eq!(bin(&["analyze", raw.to_str().unwrap(), "--out", out.to_str().unwrap()]).0, 0);
            dirs.push(out.to_str().unwrap().to_string());
        }
    }
    let mut args = vec!["compare", "--a", "A", "--b", "B", "--out"];
    let rep = d.join("cmp.json");
    args.push(rep.to_str().unwrap());
    args.extend(dirs.iter().map(|s| s.as_str()));
    let (code, stdout, err) = bin(&args);
    assert_eq!(code, 0, "{err}");
    assert!(stdout.contains("[processes] A 5 次，B 5 次，顺序 ABABABABAB"), "{stdout}");
    assert!(stdout.contains("cpu_mean_pct") && stdout.contains("B lower: ranges do not overlap"), "{stdout}");
    assert!(rep.exists());
    let (code, _, err) = bin(&args);
    assert_eq!(code, 2, "report must not be overwritten");
    assert!(err.contains("exists"), "{err}");

    let raw = d.join("rawX");
    raw_run(&raw, "B", 99, 40, 1000);
    let x = d.join("X");
    bin(&["analyze", raw.to_str().unwrap(), "--out", x.to_str().unwrap()]);
    let mut bad: Vec<&str> = vec!["compare", "--a", "A", "--b", "B", dirs[0].as_str()];
    bad.push(x.to_str().unwrap());
    let (code, _, err) = bin(&bad);
    assert_eq!(code, 2);
    assert!(err.contains("incompatible") && err.contains("refresh=1000"), "{err}");
}

#[test]
fn invalid_input_is_rejected() {
    let d = tmp("invalid");
    assert_eq!(bin(&[]).0, 2);
    assert!(bin(&["sample", "--mode", "fullscreen"]).2.contains("--mode must be one of"));
    assert!(bin(&["sample", "--bogus", "1"]).2.contains("unknown option --bogus"));
    let base = ["sample", "--label", "A", "--app-version", "0.8.2", "--mode", "tray", "--refresh-ms", "2000", "--helper", "current"];
    let mut a = base.to_vec();
    a.extend(["--interval-ms", "50", "--out", "x"]);
    assert!(bin(&a).2.contains("--interval-ms must be"));
    let mut a = base.to_vec();
    a.extend(["--setting", "noequals", "--out", "x"]);
    assert!(bin(&a).2.contains("KEY=VALUE"));
    let mut a = base.to_vec();
    let exists = d.to_str().unwrap();
    a.extend(["--out", exists]);
    assert!(bin(&a).2.contains("already exists"));
    let mut a = base.to_vec();
    let fresh = d.join("new");
    a.extend(["--out", fresh.to_str().unwrap()]);
    let (code, _, err) = bin(&a);
    assert_eq!(code, 2);
    if !cfg!(windows) {
        assert!(err.contains("only on Windows"));
        assert!(!fresh.exists());
    }
    std::fs::create_dir(d.join("broken")).unwrap();
    std::fs::write(d.join("broken/run.json"), "{not json").unwrap();
    assert!(bin(&["analyze", d.join("broken").to_str().unwrap(), "--out", d.join("o").to_str().unwrap()]).2.contains("run.json"));
    assert!(bin(&["compare", "--a", "A", "--b", "B", d.join("missing").to_str().unwrap()]).2.contains("summary.json"));
}
