# Per-process disk speed and GPU (0.8.3)

Processes shows which process is reading or writing, and which one uses the
GPU, next to the existing CPU and memory columns. Whole-machine Disk and GPU
on the dashboard, mini window and status bar are unchanged.

## Disk read and disk write

- Unit: bytes per second, shown as B/s, KB/s, MB/s or GB/s (1024 steps).
- Interval: the bytes a process read or wrote between two captures, divided
  by the wall time between them. Captures follow the refresh interval
  (3 s by default). There is no extra walk over processes.
- Source: sysinfo's cumulative per-process totals. On Windows these are
  `GetProcessIoCounters` ReadTransferCount and WriteTransferCount. They count
  every read and write call, including file reads served from cache and I/O
  to devices other than disks, so they can exceed what the physical disk did.
  On Linux they are `read_bytes` and `write_bytes` from `/proc/<pid>/io`.
- Identity: pid plus start time. A reused pid starts a new baseline.
- A dash (no reading) means one of these, never idle:
  - the first capture of a process, or the first one after a pause or any
    gap longer than 2.5 refresh intervals,
  - both counters read 0, which is how sysinfo reports counters it could not
    read (access denied); a process that truly never did I/O also shows a dash,
  - a counter went backwards.
- `0 B/s` means two real readings with no growth.

## GPU %

- Windows: `\GPU Engine(*)\Utilization Percentage` from PDH, the counter Task
  Manager uses, read once per capture in the same query as the adapter totals.
  No admin rights and no sensor helper are needed. A process's GPU % is its
  busiest single engine on any adapter. All engine types count (3D, Copy,
  VideoDecode, VideoEncode, Compute and others the driver reports).
- A process with no engine instance has no GPU context and reads 0.0%.
- A dash means the whole sample has no reading: the first capture, PDH
  unavailable, or no GPU engines reported. Fob never fills that with 0.
- Instance names carry only the pid. A pid reused between two captures could
  in theory be charged the old process's value. PDH should report a counter
  that restarts as a negative-value error, which Fob drops; this is not
  verified on Windows.
- Linux keeps DRM fdinfo per process for readable processes only.

## Dedicated VRAM

- Windows only, hidden by default: `\GPU Process Memory(*)\Dedicated Usage`
  in bytes, summed over adapters, in the same PDH query. Shared GPU memory is
  not shown. A dash means that counter is unavailable.

## Settings

`behavior.processColumns` is 2. Settings saved without it get GPU % shown once
(`preferences.rs` `migrate_columns`); hiding it afterwards sticks. Disk read and
disk write are new keys and default to shown. The old `disk_usage` key is dropped.
The process table is about 1306 px wide with the default columns, so at the
default 1280 px window the Run time column needs a short horizontal scroll.

## Not verified on Windows

PDH GPU Engine and GPU Process Memory values, GetProcessIoCounters behavior for
protected or elevated processes, the table layout in WebView2, and the CPU cost
of the added memory counter were not run on Windows.
