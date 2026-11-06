//! Compiles the project's own monitoring sources (no copies), so their
//! #[cfg(test)] tests run with `cargo test` here. `super::` in those files
//! resolves to this crate root.
#[path = "../../../src-tauri/src/monitoring/process_monitor.rs"]
mod process_monitor;
#[path = "../../../src-tauri/src/monitoring/types.rs"]
mod types;
pub use process_monitor::ProcessMonitor;
pub use types::*;
