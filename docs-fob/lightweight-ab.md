# Lightweight A/B sampler (tools/fob-perf)

Standalone, read-only Windows CLI that measures a running Fob's CPU and memory so the
lightweight pass (docs-fob/lightweight.md) can be judged on controlled data. It does
not ship inside Fob and changes no app code, version, helper protocol or setting.

## Design

- Architecture. Native Rust CLI, its own crate and workspace. Pure core (`model`,
  `tree`, `collect`, `analyze`, `compare`, `store`) builds and tests on Linux. Only
  `win.rs` touches Windows APIs. Cross-built with the existing cargo-xwin toolchain and
  cached crates (serde, serde_json, sha2, windows-sys 0.59), static CRT. PowerShell is
  not on the box, so a script could not be parsed or tested here.
- Discovery. Each sample takes one Toolhelp process list (pid, parent pid, exe name).
  No WMI, command lines or environment. The root is the only top-level `fob.exe` (or
  `--pid`). A process is identified by pid plus creation time. A descendant is counted
  only if its parent is in the tree and its creation time is not earlier than the
  parent's, which rejects stale parent pids after reuse. `fob-sensors.exe` and
  `fob-mft.exe` are excluded and listed. Unrelated `msedgewebview2.exe` is never added.
- Reads. One handle per owned pid (`PROCESS_QUERY_LIMITED_INFORMATION`, plus
  `PROCESS_VM_READ` when allowed), held across samples so the pid cannot be reused while
  held. `GetProcessTimes` gives kernel+user CPU. `K32GetProcessMemoryInfo` gives
  PrivateUsage and WorkingSetSize.
- CPU. Interval CPU % = sum over identities present in both samples of the cumulative
  CPU delta, plus the whole CPU of a child created inside the interval, divided by
  monotonic elapsed time and `GetActiveProcessorCount(ALL_PROCESSOR_GROUPS)`. The first
  sample has no delta. The run mean is weighted by elapsed time over clean steady
  intervals. Peak and P95 are per sampling interval.
- Memory. Totals are only formed when every owned process was readable. Private
  committed bytes is the primary metric. The working-set sum is reported but can double
  count shared pages. Neither is Task Manager's private working set.
- Phases. `--settle-s` after attach is reported separately (max/first private). The
  steady phase gives CPU, memory stats and a least-squares private-bytes slope (MB/h).
  `root_age_at_attach_s` records how long Fob had been running.
- Quality. `invalid`: root exited, pid reused, unreadable, discovery failed so no
  clean data, or no steady CPU/memory. `qualified`: any unclean steady interval (missing
  read, exited child whose last CPU is lost, child first seen late), a late interval
  (over 1.5 x the interval) or a short steady phase. `complete` otherwise. Counts are in
  summary.json.
- Output. A new result directory, created with `create_dir` and files written with
  `create_new`. run.json, samples.jsonl (raw), processes.csv, intervals.csv,
  summary.json, summary.txt, sampler.json (own CPU seconds and peak working set).
  The machine is a truncated SHA-256 of computer name, CPU id and core count. The exe
  is recorded by file name, size and SHA-256 only.
- Compare. Uses only `complete` runs. Rejects any mismatch in schema, machine, cores,
  interval, settle, steady, refresh, helper state, scope, exclusions or declared
  settings, unknown labels and duplicate runs. Per mode it reports n, mean, sample SD,
  min, max and B-A for CPU mean, CPU P95, private mean and drift. The verdict is
  directional only with >= 5 complete runs per side, interleaved order, and
  non-overlapping ranges (for 5 vs 5, p ~= 0.008 two-sided Mann-Whitney). Otherwise it
  is `inconclusive`. Different builds are always listed as a confounder.

## Acceptance (observable)

1. `smoke` lists Fob root and owned WebView2 children, helpers as excluded, and fails
   on unreadable processes, zero or several Fob roots.
2. Exits, reuse, access denial, missing memory and failed discovery are counted and
   downgrade the quality. None of them become zero.
3. CPU follows the formula above, including more than 64 processors.
4. Results are never written over an existing directory or report.
5. Compare rejects incompatible runs and stays inconclusive for small, noisy,
   incomplete or non-interleaved data.

## Evidence (Linux box, 2026-11-08)

- `cargo test --offline` in tools/fob-perf: core 13, compare 7, cli 3, all passing
  (dist/fobperf-tests.txt). Fixtures drive the real `collect::sample` with a
  deterministic fake process table. Covered: scope with an unrelated WebView2 and
  helper, several roots, child fob.exe, CPU normalisation (8 and 128 cores),
  time-weighted mean with a late interval, child birth and exit, root exit, root pid
  reuse, child pid reuse with a stale parent, access denied, unreadable memory,
  failed discovery, settle vs steady and drift, short run. CLI tests run the built
  binary for analyze, compare, no-overwrite, incompatible refresh, bad arguments,
  malformed input and the non-Windows refusal.
- `cargo xwin build --release --target x86_64-pc-windows-msvc` and
  `cargo xwin test --no-run` compile without warnings. PE: AMD64 console. Imports
  kernel32, ntdll, bcryptprimitives and api-ms-win-core-synch only. No VC++ runtime
  needed.
- Package dist/fob-perf-v0.1.0-windows.zip: fob-perf.exe, 使用说明.md,
  SHA256SUMS.txt. Zip CRC test, extraction, checksum check and byte equality with the
  build output pass.

Not verified: no Windows run. Toolhelp, handle access to WebView2 children, real
helper exclusion, the sampler's own overhead and every Fob reading are unverified until
a pilot runs on Windows.

## Comparison limits

- No 0.8.1 package is in dist/. 0.8.1 vs 0.8.2 is not an isolated lightweight change.
  0.8.2 also adds the Settings helper card and a helper bundle change, and a 0.8.1 app
  with an installed 0.8.2 helper may behave differently. A version-only difference is
  therefore not causal. A clean baseline would be a build of the same source with only
  the lightweight commit reverted. Making that build needs separate approval.
- Full method: 4 modes x 5 runs per side, interleaved, 120 s settle + 600 s steady is
  about 8 hours of machine time. Start with a pilot. Choose the modes and the baseline
  after the pilot instead of assigning the full matrix by default.
- The tool does not measure wakeups, GPU or disk I/O.
