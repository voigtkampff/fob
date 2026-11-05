---
name: verify-fob
description: Drive the real Fob desktop app (Tauri, Linux debug build on the box's X display :55) to prove a feature works, mainly the tray mini window (drag, position restore, opacity, hotkey), first launch, and the process-tab cards. Use before claiming a Fob change works or before packaging a release.
---

# Verify Fob

Fob ships for Windows, but the same Rust and Svelte code runs as a Linux
debug build on the box. Everything below drives that build on the X display
`:55` (Xvfb plus xfwm4, already running). What it cannot show is listed under
Gotchas; say so in any report.

All commands go through `bin/fob-verify.sh <command> <name>`. `<name>` is an
isolated run: its own HOME at `/workspace/scratch/verify/<name>`, so several
runs never share settings. Evidence goes to `/workspace/scratch/verify/evidence/`
and survives `stop`.

## Launch

```bash
V=.cursor/skills/verify-fob/bin/fob-verify.sh
$V build app                     # npm build + cargo debug build -> /workspace/scratch/verify/winperf-app
SETTINGS='{"behavior":{"language":"zh","startInTray":true,"menuBarEnabled":true,"miniPinned":true}}' \
  $V launch drag WINPERF_SHOW_MINI=1     # prints "ready pid N" once a Fob window is visible
$V relaunch drag WINPERF_SHOW_MINI=1     # restart with the same HOME (saved state kept)
```

- No `SETTINGS`: a first launch (no settings.json).
- Rebuild after every UI change: the frontend is embedded in the binary.
  `build` touches `main.rs` so cargo re-embeds it.
- Unset `BIN` in your shell unless you mean it. A stale exported `BIN`
  silently runs an old binary.
- Debug-only env: `WINPERF_SHOW_MINI=1` (show the mini window at launch),
  `WINPERF_FAKE_SENSOR_SETUP=enable|ready`, `WINPERF_FAKE_SENSORS=<jsonl>`,
  `WINPERF_FAKE_OS`, `WINPERF_FAKE_CPU`, `WINPERF_FAKE_GPU=1` (always set),
  `WINPERF_FAKE_THREADS=N` (per-core layouts at N threads).

## Doctor

`$V doctor <name>` prints whether our pid is alive, each visible window with
title and geometry, and the files in the config and log folders. Run it first
when anything looks off. `pgrep -af winperf-app` shows which binary runs.

## Drive

- Window ids: `$V windows <name>` (titles `Fob`, `Fob mini`, `Settings`/`设置`).
- Pointer: `xdotool mousemove X Y mousedown 1 ... mouseup 1`, `click --repeat 2`.
  A real drag needs a press, a small move, a pause (0.5 s) and then steps.
- Saved files: `$V file <name> settings.json`, `$V file <name> mini-position.json`,
  Placement debug logs were removed in 0.8.0; use `mini-position.json` and window geometry instead.
- Screenshot: `$V shot <name> out.png` (whole display, 1600x1000).

Feature journeys: `features/README.md`.

## Evidence

Prove the user path: pointer on the real window, then the saved file and the
log line, then a restart that reads them back. A screenshot alone is not
proof of a saved state.

## Cleanup

`$V stop <name>` kills only the pid this script started. Evidence stays.
