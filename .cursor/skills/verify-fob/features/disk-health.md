# Disk health

The Disk detail's Physical disks section (`DiskHealth.svelte`) calls the
`disk_health` command when it opens and again at most every 5 minutes while
it stays open. The command (`src-tauri/src/disk_health/`) caches one report
for 5 minutes and never runs two scans at once.

## Browser preview (mocked readings)

```bash
npm run build
node .cursor/skills/verify-fob/bin/disk-health.mjs /workspace/scratch/verify/evidence preview
```

Renders the real `CardDetail` (disk) with generic replies from
`tests/fixtures/disk-health-fixtures.js`: `ok` (NVMe, SATA, USB without a
drive letter), `warning` (drive critical warning bits, Windows predictive
failure), `errors` (denied, controller error, short reply, a disk without a
number), `empty`, `unavailable`, `pending`, `loading` and `failed`, in zh and
en at device scale 1, 1.25 and 1.5. The main window's detail is capped at
980 px, so every window width from the 1120 px minimum up looks the same.
It fails unless the section is in the main Disk detail and not in the mini
window, opening it sends exactly one request, every state shows its note,
only the warning fixture has alerts, no label and value overlap, nothing is
clipped or past the panel edge, and no Chinese text has 。. It writes
`disk-health-<tag>-<lang>-<state>-<scale>.png` and `disk-health-<tag>.json`.

## Tests

- `src-tauri/src/disk_health/tests.rs`: NVMe log decoding (units, a
  `u128::MAX` counter, absent temperature and spare), short and malformed
  replies, the device descriptor without the serial, error classes, the
  disk/volume/WMI join, and the cache (shared scan, TTL, slow scan). On the
  box run them through `/workspace/scratch/dh-test` (`cargo test`), which
  includes `disk_health/mod.rs` by path.
- `disk_health/windows.rs` tests run on GitHub Windows only: every IOCTL needs
  no access rights, hand offsets match the `windows` crate, and the CI
  machine's own disks are probed and printed.
- `scripts/disk-health.test.mjs`: TS types match the Rust variants,
  formatting, and warnings only from real flags.

None of this reads a real NVMe or SATA disk on Windows. That needs the
owner's PC: compare with a SMART tool, check a SATA disk, a USB enclosure,
and that no UAC prompt appears.
