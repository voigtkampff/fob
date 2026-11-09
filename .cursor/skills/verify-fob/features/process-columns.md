# Process columns: disk speed and GPU

The native Linux build cannot run on the box while pkg-config, GTK and WebKit
are missing, so this journey drives the real `ProcessTable` in a browser with
mocked readings and the production CSS (rebuild with `npm run build` first).

```bash
node .cursor/skills/verify-fob/bin/process-table.mjs zh /workspace/scratch/verify/evidence
node .cursor/skills/verify-fob/bin/process-table.mjs en /workspace/scratch/verify/evidence
```

Each run clicks the GPU %, disk read and disk write headers with real pointer
events and fails unless:

- the busiest process sorts first and rows with no reading sort last both ways,
- no reading shows a muted dash with its own tooltip, and idle shows `0 B/s`,
- headers read 硬盘读取 / 硬盘写入 (zh) and Disk read / Disk write (en).

It writes `process-table-<lang>.png`, `process-table-<lang>-sorted-disk-read.png`
and `process-table-<lang>.json` (row order and the table's scroll width).

Real counter math is covered elsewhere: `cargo test` and
`cargo run --release --example disk_rates` in `tools/monitoring-harness`
(Linux sysinfo counters), and `gpu/engine_counters.rs` tests for the PDH
parsing. None of this shows Windows PDH or GetProcessIoCounters at runtime.
