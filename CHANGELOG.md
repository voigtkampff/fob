# Changelog

## 0.8.4

- On hybrid CPUs the CPU views split the load into performance and efficiency cores, read from the efficiency class Windows reports for each core. Intel shows P 核 / E 核 (P-cores / E-cores); AMD and other vendors show 性能核 / 能效核 (Performance cores / Efficiency cores)
- Each group is the average of its own threads, like the total CPU %, so a P-core's two threads count once each and no group goes past 100%
- The dashboard CPU card groups its bars and lists each group's load; the Processes CPU card, the CPU detail and the CPU details sheet show every group with its load and its cores and threads
- CPUs with one kind of core, and machines where the layout cannot be read, look as before. A group without a reading shows a dash, not 0%
- The mini window card is unchanged; its CPU detail shows the split
- The bundled sensor helper is unchanged at 0.8.2

## 0.8.3

- Processes shows each process's disk read and write speed in bytes per second over the last refresh, in two sortable columns (硬盘读取 / 硬盘写入). A process without two readings, or whose counters cannot be read, shows a dash instead of 0 B/s
- GPU % is visible by default, once, for settings saved by earlier versions. On Windows it is the busiest GPU engine the process used, like Task Manager; a process with no GPU engine reads 0.0%, and a dash means Windows gave no GPU reading
- Optional Dedicated VRAM column (专用显存) on Windows from the same GPU counters
- No reading sorts after every real value in both directions
- The sensor helper's Disable note says PawnIO is uninstalled in Windows Settings, not in Fob
- The bundled sensor helper is unchanged at 0.8.2, so an installed 0.8.2 helper needs no update

## 0.8.2

- Cache command and environment details by process identity, share environment lists between frames, and discard exited-process cache entries without changing JSON shape

- Add persistent sensor helper controls in Settings › General, with installed and bundled versions, read-only status refresh, and explicit Install, Update, Repair, and Disable consent
- Reuse the existing verified helper setup flow and disclose whole-Disk metadata access without elevating Fob
- Detect mixed helper bundles and missing task or sensor response without claiming the helper is working, and prevent concurrent setup prompts across app windows

## 0.8.1

- Normally launched Fob uses the updated elevated sensor helper for NTFS MFT space scans. The helper update uses the existing explicit one-time UAC flow.
- Independent, current-user scan IPC keeps sensor updates separate. Requests only authorize a fixed NTFS drive root and return bounded read-only metadata. Cancellation, disconnects, and deadlines stop the worker.
- Old or unavailable helpers retain direct-MFT and SSD-aware folder-scan fallback. Scan results identify the method used.

## [1.3.0] — 2026-10-29

### A refreshed desktop, with the themes you know

- Migrate the interface to shared shadcn-svelte components and consistent spacing, controls and semantic theme tokens; retain all 24 original themes.
- Add native process/app icons on macOS, Linux and Windows, with the familiar ghost when an icon cannot be resolved.
- Add keyboard/pointer inspection and faded area fills to CPU, memory, GPU and network charts, with approximately two minutes of in-memory history and honest gaps for paused or missing readings.
- Add detailed CPU, memory, graphics, storage, system and network sheets, including per-core activity, volumes and interface search.
- Simplify the app bar and process toolbar; add separate Settings and About windows, automatic preference migration, cross-window synchronization, and native desktop shortcuts.

### Processes and graphics

- Add expandable process families with combined CPU/RAM, matching-parent context, stable family pagination and saved List/Tree choice.
- Search local TCP listeners and bound UDP sockets with `:3000` or `port:443`. Socket scans run only during explicit port queries; bare numbers continue to match PIDs. Thanks to [@felipe-vicente](https://github.com/felipe-vicente) in [#201](https://github.com/Abdenasser/neohtop/pull/201).
- Add driver-dependent GPU utilization, memory, temperature, core count, power and clocks. Use macOS IORegistry, Linux AMD sysfs/NVIDIA NVML, and Windows NVML/DXGI inventory; unsupported readings remain unavailable.
- Integrate optional per-process GPU engine accounting and filters for supported Linux DRM drivers. Readings need a baseline, may exceed 100% across engines and never treat inaccessible data as zero. Thanks to [@nezmaka](https://github.com/nezmaka) in [#202](https://github.com/Abdenasser/neohtop/pull/202).

### Menu-bar companion

- Add a rounded overview with CPU, RAM, GPU, network, free storage and the five busiest processes, with real app icons and inspector shortcuts.
- Share the sample, interval, pause and theme across windows; stop companion polling while hidden.
- Offer tray visibility and opt-in monitoring after closing the main window. Closing Settings or About leaves the monitor running; Quit always exits.

### Fixes and delivery

- Handle zero/unknown storage capacity without NaN percentages. Thanks to [@dajiaohuang](https://github.com/dajiaohuang) in [#204](https://github.com/Abdenasser/neohtop/pull/204).
- Fix process-inspector clipping and bottom spacing, dialog/empty-state consistency, theme contrast and focus recovery.
- Correct network-rate baselines for new/reset interfaces and report missing driver counters honestly.
- Repair cross-platform packaging and coordinate release publication only after all five platform jobs pass.
- Ship Apple Silicon/Intel macOS DMGs, Linux x86_64/ARM64 deb/rpm/AppImage packages, and a Windows x64 NSIS installer, with SHA-256 checksums and a build manifest. macOS release builds require signing and notarization.
- Update the website, screenshots, installation guide, usage documentation and machine-readable product references.

### Privacy

- Add a disclosed basic launch counter (app version, no persistent identifier) through a Cloudflare relay. Detailed feature usage remains off by default in Settings → Privacy. Process names, commands, queries, paths, port numbers and system readings are never sent.
- Keep Google Analytics credentials on the server. Builds without the analytics endpoint send no desktop events; monitoring works offline.

### Compatibility and scope

- Source builds now require Rust 1.88 or newer and Node.js 22. GPU/socket detail visibility depends on drivers and operating-system permissions; Linux tray behavior varies by desktop.
- GPU shared memory on Apple silicon is part of system RAM. Family RAM totals can double-count shared pages. Short resource histories are in-memory and reset on restart.
- NeoHtop Pro is postponed. This is the free, MIT-licensed desktop app; no account, paid plan or cloud subscription is required.

[1.3.0]: https://github.com/Abdenasser/neohtop/compare/v1.2.0...v1.3.0

Older release notes are available on [GitHub Releases](https://github.com/Abdenasser/neohtop/releases).
