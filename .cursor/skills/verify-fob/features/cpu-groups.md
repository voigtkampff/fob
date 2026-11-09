# CPU core groups: P-cores / E-cores

The native Linux build cannot run on the box while pkg-config, GTK and WebKit
are missing, and the box CPU reports no core groups anyway, so this journey
drives the real CPU views in a browser with mocked readings and the
production CSS (rebuild with `npm run build` first).

```bash
node .cursor/skills/verify-fob/bin/cpu-views.mjs zh /workspace/scratch/verify/evidence
node .cursor/skills/verify-fob/bin/cpu-views.mjs en /workspace/scratch/verify/evidence
```

Each run renders the dashboard CPU card, the mini window card and its CPU
detail, the process-tab CPU card at 144 px and 200 px, the CPU detail and the
CPU details sheet for five layouts, and fails unless:

- example hybrid CPU (Intel hybrid): P 核 / E 核 (P-cores / E-cores) with each group's
  average, bars grouped 16 + 8, detail headings `8 核 / 16 线程` and
  `8 核 / 8 线程`, total unchanged,
- Ryzen AI 9 HX 370 (AMD hybrid): 性能核 / 能效核 (Performance cores /
  Efficiency cores), never P / E,
- uniform CPU and unread layout: today's views, no group names or headings,
- a group with no reading shows a dash, never 0%,
- the mini window card keeps its one-line footer and its detail fits 600 px.

It writes `cpu-views-<lang>-<layout>.png` (whole page),
`cpu-views-<lang>-<layout>-<section>.png` and `cpu-views-<lang>.json`.

Topology parsing is covered by `cargo test` in `tools/monitoring-harness`
(`cpu_topology` tests) and the grouping math by
`node --test scripts/cpu-groups.test.mjs`. None of this shows what
GetSystemCpuSetInformation returns on a real Windows machine.
