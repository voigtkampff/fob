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

Renders the default columns and every column from a full tree view, with a
row of the longest value each column shows (`100.0%`, `24575.9 MB`,
`1024 KB/s`, `1023.9 GB`, a 7-digit PID) and run times from `0h 37m 45s` to
`9999h 59m 59s`, in zh and en, flat and tree, at 1024, 1280, 1400, 1480,
1536, 1600 and 1920 px (1920, 1536 and 1280 are a 1920 px screen at 100%,
125% and 150% scaling). It fails when a header is clipped or has under 5 px
to spare, a number is truncated or needs more than its width minus 5 px, a
header's text edge is off its cells' text edge, the last column's text comes
within 8 px of the sticky actions column (at rest when the table fits,
scrolled to the end when it does not), or the table scrolls sideways while
the window is at least its minimum width, or the name column is wider than
its widest (`nameMaxWidth` in `utils/table-layout.js`: 330 px, 560 px in tree
view, room for the truncated name plus icons and the deepest indent). The
name column takes the space left up to that widest and narrows to 200 px
(280 px in tree view) before the table scrolls; any width beyond goes to an
empty spacer cell after the actions column.
It writes `columns-<tag>-<lang>-<view>-<set>-<width>[-right].png` and
`columns-<tag>.json` with each column's width, header need and value need,
the table's minimum width and the gap to the actions column.
`scripts/process-columns.test.mjs` pins one width per value kind (id,
percent, rate, bytes, duration), the run time format (`7h 6m 32s` in both
languages) against Consolas and Liberation Mono widths, and the table's
minimum width.
