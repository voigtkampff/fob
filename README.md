English | [中文](README.zh.md)

# Fob

Fob is a performance monitor for Windows 10 and 11 (x64). It is a fork of
[NeoHtop](https://github.com/Abdenasser/neohtop) by Abdenasser (MIT, see
[LICENSE](LICENSE)). Fob adds a card dashboard, a tray mini window, a status
bar overlay, a disk space map, and an optional sensor helper on top of
NeoHtop's process monitor.

Fob was called winperf before. The Rust crate and some internal names still
use `winperf`. Everything that ships is called Fob: `fob.exe`,
`fob-sensors.exe`, and `fob-mft.exe`.

Current version: 0.8.4 (see [CHANGELOG.md](CHANGELOG.md)). The Windows
package `fob-v0.8.4-x86_64-windows.zip` is on the
[v0.8.4 release page](https://github.com/voigtkampff/fob/releases/tag/v0.8.4).
To build from source instead, see
[Build the Windows package](#build-the-windows-package).

> Screenshots: to be added. The `screenshot.png`, `screenshot-light.png`,
> and `docs/screenshots/` images in this repository show NeoHtop, not Fob.

## What Fob shows

The main window has three tabs: **Dashboard**, **Processes**, and
**Space map**.

- **Dashboard.** Cards for CPU, GPU, memory, free space on the system drive,
  network, temperatures and fans, and the top processes by memory. Click a
  card to open its detail view:
  - CPU: per-thread load and top processes.
  - GPU: per-engine load and top GPU processes.
  - Memory: usage with commit charge.
  - Drives: all drives.
  - Network: per-app speeds and adapters.
  - Temperatures & fans: all sensors.
- **CPU core groups (0.8.4).** On hybrid CPUs, the CPU views split the load
  into performance and efficiency cores, from the efficiency class that
  Windows reports for each core. Intel CPUs show P-cores and E-cores. AMD and
  other vendors show Performance cores and Efficiency cores. Each group is
  the average of its own threads, so a group never goes past 100%. CPUs with
  one kind of core look as before.
- **GPU load.** Fob reads the Windows PDH counter
  `\GPU Engine(*)\Utilization Percentage` per adapter and shows the busiest
  engine, as Task Manager does. NVIDIA cards are also read through NVML.
- **Processes.** NeoHtop's process table, with columns for **Disk read**
  and **Disk write** in bytes per second, **GPU %**, and an optional
  **Dedicated VRAM** column (0.8.3). A dash means that Fob has no reading.
  It never means 0. See [docs/process-disk-gpu.md](docs/process-disk-gpu.md)
  for the sources and limits of each column.
- **Space map.** A treemap of what fills a drive. Choose a drive and press
  **Scan**. Click a folder to open it. Right-click an item and choose
  **Show in File Explorer**. Alt+Left and Alt+Right go back and forward.
  Fob scans a drive only when you press **Scan**.
- **Tray mini window.** Left-click the tray icon to open a 360 px panel with
  the same cards. It hides when it loses focus or when you press Esc, unless
  you pin it. Drag it by its title bar to move it. Double-click the title bar
  to put it back by the tray.
- **Status bar.** An always-on-top row of readings, off by default. Clicks
  pass through it. Hold Ctrl to drag it. You choose the readings and their
  order, dock it at the top, at the bottom, or anywhere, and set its opacity
  and theme. It does not hook into games, so it cannot show FPS.
- **Global hotkeys.** You can set separate hotkeys to show or hide the mini
  window, the status bar, and the full window. All three are off until you
  set them, and each needs Ctrl, Alt, or Win.
- **Chinese and English.** Fob follows the Windows display language. You can
  change it in Settings or in the tray menu.
- **No telemetry.** NeoHtop's usage reporting was removed. Fob sends nothing
  about your system off your machine.

Not done: battery, health score, and speed test.

## Run Fob

Fob is a portable folder. It has no installer.

1. Extract `fob-v<version>-x86_64-windows.zip`.
2. Keep the `sensors` and `PawnIO` folders next to `fob.exe`.
3. Run `fob.exe`.

By default Fob starts in the tray. Closing the main window keeps Fob running
in the tray. To quit, use **Quit** in the tray menu. Fob has no native menu
bar. Settings, About, and Quit are in the title-bar menu. Ctrl+, opens
Settings.

`fob.exe` uses the WebView2 runtime that ships with Windows 10 and 11. It
runs with normal user rights.

## Turn on temperature, fan, and per-app network readings

Without the sensor helper, only NVIDIA graphics cards report temperature and
fan speed. The optional, read-only sensor helper adds CPU and motherboard
sensors, per-app network speeds, and MFT scans for Space map without
elevating Fob.

To install it, click **Enable full temperature monitoring** on the
**Temp & fan** card, or open **Settings › General › Sensor helper** and
click **Install**. Fob lists the steps first. Nothing installs until you
press **Continue** and approve one Windows permission (UAC) prompt. Then
Fob does the following:

1. Installs the PawnIO driver (v2.2.0, signed by namazso), only if it is
   missing. PawnIO opens its own installer window.
2. Copies the helper to `C:\Program Files\Fob\sensors`. Fob checks each file
   against SHA-256 hashes compiled into `fob.exe` before it copies them.
3. Creates the scheduled task `fob-sensors` through the Task Scheduler API.
   The task starts the helper as administrator now and each time you sign
   in, without asking again.

`fob.exe` itself stays unelevated. The helper serves sensor readings on the
named pipe `\\.\pipe\fob-sensors`. It reads temperatures and fan speeds with
LibreHardwareMonitorLib 0.9.6. It never changes fan speeds.

**Settings › General › Sensor helper** shows the installed and bundled
versions and offers **Install**, **Update**, **Repair**, or
**Refresh status**. **Refresh status** never asks for permission. To remove
the helper, choose **Disable…**. It stops the helper and removes it after
one permission prompt. The PawnIO driver stays installed. Uninstall it in
Windows Settings if nothing else needs it.

> **Privacy note.** While the helper runs, programs in your Windows user
> session can query file names, sizes, and folder structure across whole
> fixed NTFS drives, including folders you cannot open. The helper returns
> no file contents and makes no writes.

### How Space map scans a drive

1. If the helper is installed and current, the helper reads the NTFS Master
   File Table (MFT). Scanning never asks for permission.
2. If the helper is missing, old, or incompatible, Fob reads the MFT
   directly when Windows allows it.
3. Otherwise Fob walks the folders.

The result names the method it used, for example **MFT via helper** or
**Folder scan**. Details: [docs/mft-helper.md](docs/mft-helper.md).

## Settings

Fob saves settings automatically to `settings.json` in its app config
folder (Tauri app identifier `com.hcb.fob`). The defaults come from
[src/lib/definitions/default-config.json](src/lib/definitions/default-config.json).
Change settings in the Settings window. These `behavior` defaults are the
ones you are most likely to change:

| Key | Default | Meaning |
| --- | --- | --- |
| `refreshRate` | `3000` | Refresh interval in milliseconds, for the main window and the mini window |
| `language` | `"auto"` | `auto` follows Windows |
| `startInTray` | `true` | Start as a tray icon only |
| `keepRunning` | `true` | Keep running in the tray when the main window closes |
| `launchAtLogin` | `false` | Open Fob when you sign in to Windows |
| `miniOpacity` | `100` | Mini window background opacity, in percent |
| `overlayEnabled` | `false` | Show the status bar |
| `overlayMetrics` | `["cpu", "cpuTemp", "gpu", "gpuTemp", "ram", "net"]` | Status bar readings, left to right |
| `overlayDock` | `"top"` | Status bar position: `top`, `bottom`, or `free` |
| `overlayOpacity` | `60` | Status bar opacity, in percent |
| `miniHotkey`, `overlayHotkey`, `mainHotkey` | `""` | Global hotkeys, not set |

The status bar can also show `vram`, `disk`, and `power` (GPU power).

## Build the Windows package

Fob is cross-compiled for Windows on Linux with `cargo xwin`. The build makes
no installer. It makes one standalone exe. The VC runtime and WebView2Loader
are linked statically (`.cargo/config.toml` sets `STATIC_VCRUNTIME`).

### Requirements

- Node.js 22 (`.nvmrc`) and npm.
- Rust 1.88 or newer (`rust-version` in `src-tauri/Cargo.toml`), with
  `cargo-xwin` and the `x86_64-pc-windows-msvc` target.
- [llvm-mingw](https://github.com/mstorsjo/llvm-mingw), for `llvm-rc`.
  Without `llvm-rc`, `fob.exe` is built without its manifest, icon, and
  version resources, and Windows refuses to start it.
- A .NET 8 SDK, to build the sensor helper.
- Python 3 with `pefile` (`pip install pefile`).
- `curl`, to download the pinned PawnIO installer once.

### Build the full package

```bash
LLVM_MINGW=/path/to/llvm-mingw DOTNET=/path/to/dotnet scripts/package-windows.sh
```

`LLVM_MINGW` defaults to
`/workspace/tools/llvm-mingw-20261020-ucrt-ubuntu-22.04-x86_64`. `DOTNET`
defaults to `dotnet` on `PATH`, then `/workspace/tools/dotnet/dotnet`.

The script does the following:

1. Builds the sensor helper with `sensors-helper/build.sh`. That script runs
   the helper's wire-format tests, builds `fob-mft.exe` and
   `fob-sensors.exe`, downloads and checks `PawnIO_setup.exe`, and writes
   the hash manifest that `fob.exe` embeds.
2. Builds the frontend with `npm run build`.
3. Builds the exe with `cargo xwin build`.
4. Checks the manifest, version resources, and icon of every exe.
5. Writes `dist/fob-v<version>-x86_64-windows.zip` and
   `dist/fob-v<version>-SHA256SUMS.txt`.

The zip contains one folder with `fob.exe`, `sensors/`, `PawnIO/`,
`licenses/`, `LICENSE.txt`, `README.txt`, and `SHA256SUMS.txt`.

### Build only the exe

```bash
npm ci && npm run build    # frontend -> build/, embedded into the exe
cd src-tauri && cargo xwin build --target x86_64-pc-windows-msvc --release
# -> src-tauri/target/x86_64-pc-windows-msvc/release/winperf.exe (shipped as fob.exe)
```

Put `llvm-rc` on `PATH` first.

### Run the tests

| Command | What it covers |
| --- | --- |
| `npm run check` | Svelte and type checks |
| `npm run test:release` | `node --test scripts/*.test.mjs` and the metrics test |
| `for t in tests/*.mjs; do node "$t" \|\| exit; done` | Frontend scripts: dashboard, i18n, copy, overlay, treemap, and others |
| `cd src-tauri && cargo test` | Rust backend. On Linux it needs the webkit2gtk-4.1 and gtk3 development packages. |
| `cargo test --manifest-path tools/monitoring-harness/Cargo.toml` | Monitoring modules, without the GTK and WebKit packages |
| `cargo test --manifest-path sensors-helper/mft-worker/Cargo.toml` | MFT worker |
| `npm run format:check` | Prettier and `cargo fmt` |

`sensors-helper/build.sh` also runs the helper's C# wire tests.

## Known limits

- Microsoft Defender flagged winperf 0.4.0 as
  `Behavior:Win32/Execution.A!ml` during sensor setup. Fob 0.5.0 and later
  changed the setup steps, but the exe is unsigned and still elevates once
  and registers a logon task. See
  [docs-fob/defender-false-positive.md](docs-fob/defender-false-positive.md).
- Per-process **Disk read** and **Disk write** come from
  `GetProcessIoCounters`. They count every read and write call, including
  cached reads, so they can exceed what the physical disk did.
- With the default columns the process table is about 1306 px wide, so the
  default 1280 px window needs a short horizontal scroll.
- Some 0.8.x behavior has not yet been checked on Windows: the PDH GPU
  values per process, and the Settings install, update, and repair flow for
  the helper. See [docs/process-disk-gpu.md](docs/process-disk-gpu.md) and
  [docs/helper-settings.md](docs/helper-settings.md).

## More documentation

- [CHANGELOG.md](CHANGELOG.md): Fob releases (0.8.x) and the NeoHtop 1.3.0
  notes it builds on.
- [docs/process-disk-gpu.md](docs/process-disk-gpu.md): per-process disk and
  GPU columns.
- [docs/helper-settings.md](docs/helper-settings.md): helper controls in
  Settings.
- [docs/mft-helper.md](docs/mft-helper.md): MFT scans through the helper,
  and the security boundary.
- [tools/fob-perf/README.md](tools/fob-perf/README.md): a read-only tool that
  samples Fob's CPU and memory use on Windows (in Chinese).
- [.github/DEVELOPMENT.md](.github/DEVELOPMENT.md): NeoHtop's development
  and release guide. Its release steps describe NeoHtop's multi-platform
  workflow, not the Fob package.

## Credits and license

Fob is based on [NeoHtop](https://github.com/Abdenasser/neohtop) by
Abdenasser and is released under the MIT License. See [LICENSE](LICENSE).

The sensor helper bundles third-party components under their own licenses:
LibreHardwareMonitorLib (MPL-2.0), PawnIO (GPL-2.0), HidSharp, and
ntfs-reader. Their license texts are in
[sensors-helper/licenses/](sensors-helper/licenses/). Other third-party
license texts are in [licenses/](licenses/).
