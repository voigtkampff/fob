//! Compiles the project's own monitoring and GPU counter sources (no copies), so their
//! #[cfg(test)] tests run with `cargo test` here. `super::` in those files
//! resolves to this crate root.
#[path = "../../../src-tauri/src/monitoring/cpu_topology.rs"]
mod cpu_topology;
#[path = "../../../src-tauri/src/gpu/engine_counters.rs"]
#[allow(dead_code)]
mod engine_counters;
pub use cpu_topology::*;
#[path = "../../../src-tauri/src/monitoring/io_rates.rs"]
mod io_rates;
#[path = "../../../src-tauri/src/monitoring/process_monitor.rs"]
mod process_monitor;
#[path = "../../../src-tauri/src/monitoring/types.rs"]
mod types;
pub use process_monitor::ProcessMonitor;
pub use types::*;
