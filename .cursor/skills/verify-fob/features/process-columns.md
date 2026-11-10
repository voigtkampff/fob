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

## Column widths

```bash
node .cursor/skills/verify-fob/bin/process-column-widths.mjs /workspace/scratch/verify/evidence after
```

Renders every column from a full tree view plus a row with the longest value
each column shows (`100.0%`, `24575.9 MB`, `1024 KB/s`, `1023.9 GB`,
`9999h 59m 59s`, a 7-digit PID), in zh and en, flat and tree, at 1024 and
1600 px. It fails when a header is clipped or has under 5 px to spare, a
number is truncated, or a header's text edge is off its cells' text edge.
It writes `columns-<tag>-*.png` and `columns-<tag>.json` with each column's
width, header need and value need. `scripts/process-columns.test.mjs` pins
one width per value kind (id, percent, rate, bytes, duration).
