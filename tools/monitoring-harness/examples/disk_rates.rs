// Real sysinfo counters through the shipped ProcessMonitor: this process
// writes a known number of bytes with fsync between two captures.
use monitoring_harness::ProcessMonitor;
use std::io::Write;
use std::time::{Duration, Instant};
use sysinfo::System;

const BYTES: usize = 32 * 1024 * 1024;

fn main() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/fob-disk-rate-probe.bin");
    let mut sys = System::new();
    let mut monitor = ProcessMonitor::new();
    let window = Duration::from_millis(7500);
    let me = std::process::id();
    let mine = |list: &[monitoring_harness::ProcessInfo]| {
        let p = list.iter().find(|p| p.pid == me).unwrap();
        (p.disk_read, p.disk_write)
    };
    let unknown = |list: &[monitoring_harness::ProcessInfo]| {
        list.iter().filter(|p| p.disk_read.is_none()).count()
    };
    sys.refresh_all();
    let first = monitor.collect_processes(&sys, window).unwrap();
    let t0 = Instant::now();
    println!(
        "first capture: {} processes, {} without a disk reading, probe {:?}",
        first.len(),
        unknown(&first),
        mine(&first)
    );
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&vec![7u8; BYTES]).unwrap();
    file.sync_all().unwrap();
    std::thread::sleep(Duration::from_secs(3).saturating_sub(t0.elapsed()));
    sys.refresh_all();
    let second = monitor.collect_processes(&sys, window).unwrap();
    let elapsed = t0.elapsed().as_secs_f64();
    println!(
        "second capture after {elapsed:.2} s: {} without a disk reading, probe {:?}",
        unknown(&second),
        mine(&second)
    );
    println!(
        "expected probe write about {:.0} B/s ({} bytes / window)",
        BYTES as f64 / elapsed,
        BYTES
    );
    std::thread::sleep(Duration::from_secs(3));
    sys.refresh_all();
    let third = monitor.collect_processes(&sys, window).unwrap();
    println!("third capture, no writes: probe {:?}", mine(&third));
    let _ = std::fs::remove_file(path);
}
