# Lightweight sampler audit

`tools/fob-perf` is a standalone read-only Windows CLI. It changes no Fob code,
settings, helper, driver or existing app package. The deliverable is a test kit,
not a validated Windows performance result. See its Chinese README for the short pilot.

## Data flow and contracts

Toolhelp lists pid, parent pid and executable name. Selection binds one top-level
`fob.exe` to its pid and creation time, then follows only `msedgewebview2.exe`
children through verified live WebView parents. A child's creation must not precede
its parent's. Arbitrary descendants and unrelated WebViews are not exported as
measured processes. Sensor and MFT helpers are explicitly excluded, including
helpers found as separate elevated tasks. The scope therefore excludes their costs.

Windows handles request only `PROCESS_QUERY_LIMITED_INFORMATION`. Microsoft documents
that right as sufficient for GetProcessTimes and GetProcessMemoryInfo on supported
modern Windows. Memory failures preserve the OS error. Enumeration accepts only
ERROR_NO_MORE_FILES as normal termination. Handles close when no longer selected
and on drop. The root's executable basename is checked through its image path.

Schema 2 records a monotonic timestamp for each CPU counter read. Native reads use
the midpoint around GetProcessTimes and the exit-state check, before memory reads.
For each process present and readable at both ends, CPU rate is its CPU delta divided
by its own read-to-read elapsed time. Rates sum and divide by all active logical
processors from GetActiveProcessorCount(ALL_PROCESSOR_GROUPS), including >64 cores.
The first sample is a baseline. New children require another sample, rather than
assuming their whole CPU lifetime fits a wall-clock interval. Wall clock is metadata,
not a CPU denominator or birth-time delta. Samples are sequential, not atomic.

The loop schedules settle and end boundaries as well as the normal cadence. CPU
steady means use clean intervals starting after settle, weighted by sample cadence.
A missing interval is not forgiven as a full interval of tolerance. Boundary jitter
up to 20 ms is tolerated. Acquisition skew above 5% of the requested interval,
late intervals above 1.5 times cadence, missing reads, missing sequence numbers,
insufficient coverage or membership changes disqualify a complete run. Root loss
invalidates it. Memory requires all selected processes. PrivateUsage is primary,
working-set sums include shared pages. Memory means are sample means and slope is
least-squares MB/h, not evidence of a leak. Per-process CPU seconds can be partial
when membership or reads fail; only clean intervals enter the aggregate CPU mean.

Snapshots cannot discover processes born and exited between samples. An orphan
whose parent exited cannot be attributed from the current chain. Complete means
internally complete observed data, not proof of exhaustive lifetime accounting.
Creation-time ordering assumes Windows creation timestamps remain meaningful across
clock changes. Strictly monotonic per-counter timestamps prevent wall-clock changes
from inventing CPU. Actual overhead and sandbox access still need the Windows pilot.

## Output and comparison

Each result uses a new directory and create-new files. I/O and flush errors fail the
command. Raw samples survive interruptions but an interrupted directory without
run.json and reports is not a finished run. Metadata is finalized once at the end.
The executable is hashed with a bounded buffer at attach and after sampling. Changed
or unavailable hashes lose comparison eligibility. Hashes describe the on-disk file,
not a measured app version resource or loaded memory image. Declared app version,
mode, refresh, helper and settings are labeled as declarations, not read automatically.

Output is run.json, samples.jsonl, processes.csv, intervals.csv, summary.json,
summary.txt and, when readable, sampler.json. Sampler CPU seconds and peak working set
are separate and exclude report generation, while including attach, hashing and
sampling. They are excluded from Fob's metrics. No wakeup claim is possible.
Machine identity is a truncated hash of machine name, CPU identifier and core count.
Those source identifiers, command lines, environment variables and app documents
are not exported. Reading the executable for its hash is the only app-file content
read. There is no network upload.

Analyze validates metadata, sample ordering and bound root identity. Compare
recomputes from raw data and rejects summaries that disagree. It rejects schema,
tool version, machine, core/group topology, interval, phase duration, declared refresh,
helper, scope, exclusions and setting mismatches. Duplicate run IDs, unknown or equal
labels, inconsistent versions/hashes/sizes within a label, malformed root metadata
and incomplete metrics claiming complete are rejected. Different modes form separate
groups. Within each mode, chronological runs must not overlap and must strictly
alternate. Each side needs at least five complete runs. Incomplete input or overlapping
value ranges produces an inconclusive result. Directional results never establish
a causal lightweight benefit. Different builds include all differences between them.

## Verification boundaries

Local regressions reproduce the initial wrong descendant scope, one-interval short
run being complete, sequence gaps, equal labels and mixed builds within a label.
Additional tests cover per-counter timing, failed smoke discovery/root, memory errors,
birth/exit/reuse, wall-clock changes, 128-core normalization, settle separation,
source/summary mismatch, duplicate inputs, bad flags, no overwrite, overlapping
chronology and missing comparison metrics. Tests execute the real core and CLI with
a deterministic fake process table on Linux. They do not execute Win32 APIs.

The final audit keeps actual test, build and package output in dist/fob-perf-audit-*.txt.
Cross-building and Windows test-no-run check compilation only. The box has no Windows
runtime. The parent will run the approved short pilot on the user's Windows machine
after package review, without changing settings, drivers or helpers. No Fob CPU,
memory or lightweight-benefit conclusion has been measured here.

Microsoft API references checked during this audit:

- https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes
- https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-process32firstw
- https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-process32nextw
- https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getactiveprocessorcount

## Baseline limits

No 0.8.1 package is present in dist. 0.8.1 versus 0.8.2 is not an isolated lightweight
change because the helper card and bundle also changed. An equivalent local source
baseline with only the lightweight change reverted can be prepared reversibly without
an extra approval gate. Running it on a user machine or changing the app remains a
separate action. This audit does not prepare or run that baseline. Start with one
90-second pilot, then choose relevant modes and repeats from the evidence, rather
than assigning the full roughly eight-hour matrix to the user.
