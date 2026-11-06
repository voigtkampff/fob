# Lightweight pass 1 (perf/lightweight-20261106)

Hot path: `Hub::capture` -> `ProcessMonitor::collect_processes`, once per sample
interval (1.5–30 s) from the background sampler, whether or not a window is open.

Waste found: every collection copied each process's full command line and
environment (OsString -> String) even though sysinfo fills both once per process,
and the static cache was keyed by pid only and never pruned (a reused pid kept the
old name/command; entries for exited processes stayed forever).

Fix: cache keyed by (pid, start time), pruned to live processes; command and
environment copied only until cached non-empty; environment shared as `Arc<[String]>`
(same JSON).

Microbenchmark (Linux box, 8 vCPU, 1344 processes, 72,800 env strings, release,
collect only, refresh excluded, 7 alternated runs x 300 iterations, host shared
with another build): base median 7.58 ms (7.06–8.15), new 1.27 ms (1.16–1.58)
per collection. Not a Windows or end-to-end measurement.

Next: measure Fob CPU/private bytes on Windows idle (tray only, mini, overlay,
main on Processes) before/after; then look at `get_processes` serialising every
process's environment to the main window each tick (details modal only needs one).
