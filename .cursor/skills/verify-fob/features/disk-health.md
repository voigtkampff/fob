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
`tests/fixtures/disk-health-fixtures.js`: `ok` (NVMe, a SATA SSD with every
ATA statistic, a SATA HDD with some, USB without a drive letter), `warning`
(drive critical warning bits, Windows predictive failure on a SATA disk
Windows reports asleep), `errors` (NVMe denied, controller error, short
reply, SATA denied, unsupported, short reply and error 1117, a disk without
a number), `empty`, `unavailable`, `pending`, `loading` and `failed`, in zh and
en at device scale 1, 1.25 and 1.5. The main window's detail is capped at
980 px, so every window width from the 1120 px minimum up looks the same.
It fails unless the section is in the main Disk detail and not in the mini
window, opening it sends exactly one request, every state shows its note,
the SATA values show with their units (°C, %, TB, hours, counts), the
partial and sleeping notes show, no SATA disk says it shows the Windows
status only,
only the warning fixture has alerts, no label and value overlap, nothing is
clipped or past the panel edge, and no Chinese text has 。. It writes
`disk-health-<tag>-<lang>-<state>-<scale>.png` and `disk-health-<tag>.json`.

## Row hover (mocked readings)

```bash
npm run build
node .cursor/skills/verify-fob/bin/disk-health-hover.mjs /workspace/scratch/verify/evidence hover
```

Hovering a metric row (label, the empty middle or value) gives only that
row's label and value text-sized chips in the theme's foreground color with
background-colored text, so far-apart pairs are easy to match. A warning
value keeps the same chip with a peach ring, because a peach fill misses
4.5:1 in some light palettes. Each row is a subgrid `div` inside the `dl`,
so the gap between the columns belongs to the row. The check moves a CDP
pointer to each row's label, middle and value, then away, and fails unless
exactly that row's two chips paint, no box moves, text on each chip is at
least 4.5:1, a chip contrasts with the panel at 3:1, no chip fills the row,
warning values (and only those) get the ring, nothing has a pointer cursor
or a tab stop, and the chips clear on leave. It covers every theme on the
`warning` fixture, then zh and en, `ok`, `warning`, `errors` and `long`,
emulated device scales 1 and 1.5, and panel widths 980 (the app's cap),
560 and 300 (narrower than the app allows, where English values wrap), in
catppuccin and githubLight. It writes
`disk-health-hover-<tag>-<theme>-<lang>-<state>-<width>.png` with one row
hovered, and `disk-health-hover-<tag>.json`. The same checks
(`scripts/package-smoke/hover.mjs`) run in the Windows package smoke with
the real cursor.

## Storage side panel (mocked readings)

```bash
npm run build
node .cursor/skills/verify-fob/bin/disk-health-sidebar.mjs /workspace/scratch/verify/evidence sidebar
```

Mounts the real main window route (`src/routes/+page.svelte`) from
`tests/fixtures/main-page-panel.html`, clicks the Processes tab's storage card
with pointer events and checks the Physical disks summary in the `ok`,
`warning`, `long` (long models, seven volumes, no number), `unavailable`,
`loading`, `failed` and `empty` states, zh and en, at device scale 1, 1.25
and 1.5 in a 1120 px window. It fails unless there is one entry per physical
disk, only disks whose NVMe log or ATA statistics reported them show
temperature and life used (the HDD shows temperature only), nothing is
clipped, the capacity card, chart and volumes are still there, nothing asks
before the panel opens, and exactly one 5 minute refresh timer runs while it
is open. In `ok` it clicks 查看健康详情 / View health details and checks the
panel closed, the page is not left locked, the Dashboard tab shows the Disk
detail with focus on its health title, and leaving the detail stops the
timer. In `loading` it presses Esc and checks focus returns to the card
button and the timer stops. It writes
`sidebar-<tag>-<lang>-<state>-<scale>.png`, `sidebar-<tag>-<lang>-after-link.png`
and `sidebar-<tag>.json`.

## Tests

- `src-tauri/src/disk_health/tests.rs`: NVMe log decoding (units, a
  `u128::MAX` counter, absent temperature and spare), ATA Device Statistics
  decoding (Supported and Valid bits, signed temperature and 80h, life used
  0 and 255, sector size and overflow, page header, page list count, offsets
  and lengths), which buses are asked, short and malformed replies, the device descriptor without the serial, error classes, the
  disk/volume/WMI join, and the cache (shared scan, TTL, slow scan). On the
  box run them through `/workspace/scratch/dh-test` (`cargo test`), which
  includes `disk_health/mod.rs` by path.
- `disk_health/windows.rs` tests run on GitHub Windows only: every IOCTL needs
  no access rights, hand offsets match the `windows` crate (ATA protocol and
  log page values, the access alignment descriptor), and the CI
  machine's own disks are probed and printed.
- `scripts/disk-health.test.mjs`: TS types match the Rust variants and the
  `AtaStats` fields, temperature and life used from either log, the note
  each bus gets,
  formatting, warnings only from real flags, and the shared watcher (one
  request on open, every 5 minutes, one retry after a pending reply, no
  overlapping requests, nothing after stop).

None of this reads a real NVMe or SATA disk on Windows; GitHub runners have
no SATA disk, so the ATA path is never exercised there. That needs the
owner's PC: compare with a SMART tool, check a SATA SSD and HDD, a sleeping
HDD, a USB enclosure, and that no UAC prompt appears.
