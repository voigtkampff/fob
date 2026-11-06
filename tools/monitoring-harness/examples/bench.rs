// Microbenchmark: ProcessMonitor::collect_processes on one refreshed System,
// repeated; refresh cost excluded (same for both). Prints ms per collection.
use std::time::Instant;
fn main() {
    let iters: usize = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(200);
    let mut sys = sysinfo::System::new();
    sys.refresh_all();
    let mut m = monitoring_harness::ProcessMonitor::new();
    let warm = m.collect_processes(&sys).unwrap();
    let env: usize = warm.iter().map(|p| p.environ.len()).sum();
    let t = Instant::now();
    let mut n = 0;
    for _ in 0..iters { n += m.collect_processes(&sys).unwrap().len(); }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("procs={} env_strings={} per_collect_ms={:.4} total={}", warm.len(), env, ms, n);
}
