import assert from "node:assert/strict";
import {
  formatRate,
  formatGB,
  formatPercent,
  formatUptimeShort,
  formatDuration,
  uptimeInfo,
  averageCpu,
  systemVolume,
  gpuSummary,
  topByMemory,
  series,
  SPARK_WINDOW_MS,
  chartMax,
  sparklinePath,
  topBy,
  sensorSummary,
  sensorDevice,
  cpuTempRank,
  groupByHardware,
  formatSensor,
  fanGroups,
  fanText,
  byKind,
  setupState,
  setupSteps,
  mergeNetApps,
  sortedNetRows,
} from "../src/lib/utils/dashboard.js";
import { readFileSync } from "node:fs";
import { RESOURCE_WINDOW_MS } from "../src/lib/utils/resource-history.js";

assert.deepEqual(formatRate(null), { value: "—", unit: "" });
assert.deepEqual(formatRate(-1), { value: "—", unit: "" });
assert.deepEqual(formatRate(0), { value: "0", unit: "B/s" });
assert.deepEqual(formatRate(512), { value: "512", unit: "B/s" });
assert.deepEqual(formatRate(437 * 1024), { value: "437", unit: "KB/s" });
assert.deepEqual(formatRate(4.9 * 1024 * 1024), { value: "4.9", unit: "MB/s" });
assert.equal(formatGB(24.96 * 1024 ** 3, 2), "24.96 GB");
assert.equal(formatPercent(null), "n/a");
assert.equal(formatPercent(51.6), "52");

assert.equal(formatUptimeShort(26 * 3600 + 5), "kernel up 1 day, 2 hr");
assert.equal(formatUptimeShort(3 * 86400), "kernel up 3 days, 0 hr");
assert.equal(formatUptimeShort(3 * 3600 + 4 * 60), "kernel up 3 hr, 4 min");
assert.equal(formatUptimeShort(59), "kernel up 0 min");
assert.equal(formatDuration(69 * 3600), "2 days, 21 hr");
// Fast Startup: kernel up 2 days 21 hr, but started 3 h 4 min ago.
const now = 2_000_000_000;
assert.deepEqual(uptimeInfo(69 * 3600, now - (3 * 3600 + 240), now), {
  seconds: 3 * 3600 + 240,
  sinceStart: true,
  kernel: 69 * 3600,
});
assert.deepEqual(uptimeInfo(600, null, now), {
  seconds: 600,
  sinceStart: false,
  kernel: 600,
});
// Never more than the kernel uptime, never negative.
assert.equal(uptimeInfo(600, now - 5000, now).seconds, 600);
assert.equal(uptimeInfo(600, now + 50, now).seconds, 0);

assert.equal(averageCpu([10, 20, 30, 40]), 25);
assert.equal(averageCpu([]), null);
assert.equal(averageCpu([10, NaN]), null);

const vols = [
  { mount: "D:\\", total: 2000, available: 1000 },
  { mount: "C:\\", total: 1000, available: 400 },
];
assert.equal(systemVolume(vols)?.mount, "C:\\");
assert.equal(
  systemVolume([
    { mount: "/boot", total: 1, available: 1 },
    { mount: "/", total: 9, available: 3 },
  ])?.mount,
  "/",
);
assert.equal(
  systemVolume([
    { mount: "/a", total: 1, available: 1 },
    { mount: "/b", total: 9, available: 3 },
  ])?.mount,
  "/b",
);
assert.equal(systemVolume([]), null);

assert.deepEqual(gpuSummary([]), { percent: null, name: null });
assert.deepEqual(gpuSummary([{ name: "iGPU", utilization: null }]), {
  percent: null,
  name: "iGPU",
});
assert.deepEqual(
  gpuSummary([
    { name: "iGPU", utilization: 3 },
    { name: "RTX", utilization: 40 },
    { name: "X", utilization: null },
  ]),
  { percent: 40, name: "RTX" },
);

const procs = [
  { pid: 3, memory_usage: 10 },
  { pid: 1, memory_usage: 50 },
  { pid: 2, memory_usage: 50 },
  { pid: 4, memory_usage: 5 },
];
assert.deepEqual(
  topByMemory(procs, 3).map((p) => p.pid),
  [1, 2, 3],
);
assert.equal(procs[0].pid, 3, "input is not mutated");

assert.deepEqual(series([{ cpu: 1 }, { cpu: null }, { cpu: NaN }, {}], "cpu"), [
  1,
  null,
  null,
  null,
]);
assert.equal(chartMax([[null, 10], [20]]), 1024);
assert.equal(chartMax([[5000], [null]]), 5000);

const at = (/** @type {number} */ t, /** @type {number|null} */ v) => ({
  timestamp: t,
  v,
});
const pickV = (/** @type {{v: number|null}} */ s) => s.v;
assert.equal(sparklinePath([], pickV, { width: 10, height: 10, max: 1 }), "");
// a full 120 s of samples spans the whole width
assert.equal(
  sparklinePath([at(0, 0), at(60_000, 50), at(120_000, 100)], pickV, {
    width: 100,
    height: 10,
    max: 100,
  }),
  "M0.0,10.0L50.0,5.0L100.0,0.0",
);
// a short history is drawn at its real time position: right edge, growing left
assert.equal(
  sparklinePath([at(0, 10), at(6_000, 10)], pickV, {
    width: 120,
    height: 10,
    max: 10,
  }),
  "M114.0,0.0L120.0,0.0",
);
// nulls break the line; a long gap keeps its width instead of collapsing to one slot
assert.equal(
  sparklinePath(
    [at(0, 10), at(3_000, null), at(90_000, 10), at(93_000, 10)],
    pickV,
    {
      width: 93,
      height: 10,
      max: 10,
      windowMs: 93_000,
    },
  ),
  "M0.0,0.0M90.0,0.0L93.0,0.0",
);
// samples older than the window are dropped; values above max are clamped
assert.equal(
  sparklinePath([at(0, 5), at(200_000, 200)], pickV, {
    width: 100,
    height: 10,
    max: 100,
  }),
  "M100.0,0.0",
);
assert.equal(SPARK_WINDOW_MS, RESOURCE_WINDOW_MS, "same window as main charts");

// detail views
const ps = [
  { pid: 1, cpu_usage: 5, gpu_usage: null },
  { pid: 2, cpu_usage: 50, gpu_usage: 3 },
  { pid: 3, cpu_usage: NaN, gpu_usage: 9 },
];
assert.deepEqual(
  topBy(ps, (p) => p.cpu_usage).map((p) => p.pid),
  [2, 1],
);
assert.deepEqual(
  topBy(ps, (p) => p.gpu_usage, 1).map((p) => p.pid),
  [3],
);
assert.deepEqual(
  topBy(null, (p) => p),
  [],
);

const sensors = [
  { hardware: "Example Chip", name: "CPU", kind: "temperature", value: 41 },
  {
    hardware: "Example Hybrid CPU",
    name: "Core Max",
    kind: "temperature",
    value: 55,
  },
  {
    hardware: "Example Hybrid CPU",
    name: "CPU Package",
    kind: "temperature",
    value: 52,
  },
  {
    hardware: "NVIDIA Example GPU",
    name: "GPU Core",
    kind: "temperature",
    value: 48.5,
  },
  { hardware: "Example Chip", name: "Fan #1", kind: "fan", value: 1102 },
  { hardware: "Example Chip", name: "Fan #2", kind: "fan", value: 1349 },
  {
    hardware: "Example Chip",
    name: "Fan #2 duty",
    kind: "control",
    value: 42,
  },
];
const sum = sensorSummary(sensors);
assert.equal(sum.cpuTemp?.name, "CPU Package");
assert.equal(sum.gpuTemp?.name, "GPU Core");
assert.equal(sum.fan?.name, "Fan #2");
assert.equal(sum.count, 7);
assert.deepEqual(sensorSummary([]), {
  cpuTemp: null,
  gpuTemp: null,
  fan: null,
  count: 0,
});
// NVML only: GPU temp + fan duty, no CPU temp
const nv = sensorSummary([
  {
    hardware: "NVIDIA Example GPU",
    name: "GPU core",
    kind: "temperature",
    value: 50,
  },
  {
    hardware: "NVIDIA Example GPU",
    name: "GPU fan",
    kind: "control",
    value: 30,
  },
]);
assert.equal(nv.cpuTemp, null);
assert.equal(nv.gpuTemp?.value, 50);
assert.equal(nv.fan?.kind, "control");
assert.deepEqual(
  groupByHardware(sensors).map((g) => [g.hardware, g.sensors.length]),
  [
    ["Example Chip", 4],
    ["Example Hybrid CPU", 2],
    ["NVIDIA Example GPU", 1],
  ],
);
assert.equal(formatSensor(sensors[2]), "52 °C");
assert.equal(formatSensor(sensors[5]), "1349 RPM");
assert.equal(formatSensor(sensors[6]), "42 %");

console.log(
  "PASS: dashboard formatting, volume/GPU selection, top processes, sparkline series",
);

// winperf v0.4: helper pipe sensors (same fixture the Rust and C# tests use).
{
  const kinds = { Temperature: "temperature", Fan: "fan", Control: "control" };
  const line = readFileSync(
    new URL("../src-tauri/tests/fixtures/sensors-pipe.jsonl", import.meta.url),
    "utf8",
  ).split("\n")[0];
  const pipe = JSON.parse(line)
    .sensors.filter((s) => kinds[s.type] && Number.isFinite(s.value))
    .map((s) => ({
      hardware: s.hw,
      id: s.id,
      name: s.name,
      kind: kinds[s.type],
      value: s.value,
      source: "winperf sensor helper",
    }));
  const sum = sensorSummary(pipe);
  assert.equal(sum.cpuTemp?.name, "CPU Package");
  assert.equal(sum.cpuTemp?.value, 52);
  assert.equal(sum.gpuTemp?.name, "GPU Core");
  const groups = fanGroups(pipe);
  assert.deepEqual(
    groups.map((g) => [g.hardware, g.fans.map((f) => f.name)]),
    [
      ["Example Board", ["Fan #1", "Fan #2", "Fan #3"]],
      ["NVIDIA Example GPU", ["GPU Fan 1"]],
    ],
  );
  assert.equal(fanText(pipe), "1102 · 1349 RPM");
  assert.equal(fanText(pipe, 1), "1102 RPM +1");
  assert.equal(fanText([]), "");
  const tenth = { hardware: "B", name: "Fan #10", kind: "fan", value: 900 };
  const second = { hardware: "B", name: "Fan #2", kind: "fan", value: 800 };
  assert.deepEqual(
    fanGroups([tenth, second])[0].fans.map((f) => f.name),
    ["Fan #2", "Fan #10"],
  );
  assert.deepEqual(
    byKind([
      { kind: "control", name: "Fan #2" },
      { kind: "fan", name: "Fan #1" },
      { kind: "temperature", name: "CPU" },
    ]).map((s) => s.kind),
    ["temperature", "fan", "control"],
  );
}
{
  const base = {
    supported: true,
    bundled: true,
    pawnio_installed: false,
    installed: false,
    task: "missing",
    update_available: false,
    running: false,
    fake: false,
  };
  assert.equal(setupState(null), "none");
  assert.equal(setupState({ ...base, supported: false }), "none");
  assert.equal(setupState(base), "enable");
  assert.equal(setupState({ ...base, bundled: false }), "missing-files");
  const done = { ...base, pawnio_installed: true, installed: true, task: "ok" };
  assert.equal(setupState({ ...done, running: true }), "ready");
  assert.equal(setupState(done), "starting");
  assert.equal(setupState({ ...done, update_available: true }), "update");
  assert.equal(setupState({ ...done, task: "mismatch" }), "repair");
  assert.equal(setupState({ ...base, task: "unknown" }), "repair");
  const fresh = setupSteps(base);
  assert.equal(fresh.length, 3);
  assert.match(fresh[0], /PawnIO driver/);
  assert.match(fresh[1], /C:\\Program Files\\Fob\\sensors/);
  assert.match(fresh[2], /“fob-sensors”/);
  const again = setupSteps(done);
  assert.equal(again.length, 2); // driver already there: no driver step
  assert.match(again[0], /^Replace/);
  assert.match(again[1], /^Re-create/);
}
console.log(
  "PASS: sensor helper pipe summary, fan groups, setup states and steps",
);

{
  let rows = mergeNetApps(
    new Map(),
    [
      { name: "chrome.exe", rx: 1000, tx: 10 },
      { name: "chrome.exe", rx: 500, tx: 0 },
      { name: "steam.exe", rx: 3000, tx: 0 },
      { name: "idle.exe", rx: 0, tx: 0 },
    ],
    1000,
  );
  assert.deepEqual(
    sortedNetRows(rows).map((r) => [r.name, r.rx, r.tx]),
    [
      ["steam.exe", 3000, 0],
      ["chrome.exe", 1500, 10],
      ["idle.exe", 0, 0],
    ],
  );
  // Next second: steam is quiet (kept, at 0), chrome continues.
  rows = mergeNetApps(rows, [{ name: "chrome.exe", rx: 200, tx: 0 }], 2000);
  assert.deepEqual(
    sortedNetRows(rows).map((r) => [r.name, r.rx]),
    [
      ["chrome.exe", 200],
      ["idle.exe", 0],
      ["steam.exe", 0],
    ],
  );
  // 31 s after its last traffic an app drops out.
  rows = mergeNetApps(rows, [], 31_500);
  assert.deepEqual(
    sortedNetRows(rows).map((r) => r.name),
    ["chrome.exe"],
  );
  rows = mergeNetApps(rows, [], 40_000);
  assert.equal(rows.size, 0);
}
console.log("PASS: per-app network rows group, stick for 30 s and sort");

// mini window: one fixed size; the cards view fits it; nothing resizes or moves it
{
  const {
    MINI,
    MINI_HEIGHT,
    miniCardsHeight,
    miniProcsHeight,
    fixedRows,
    miniStyle,
  } = await import("../src/lib/utils/mini-layout.js");
  const { readFileSync, readdirSync, statSync } = await import("node:fs");
  const { join } = await import("node:path");
  // The cards view is built only from fixed-height blocks: it fits, with a
  // few px of slack for DPI rounding, so it never shows a scrollbar.
  const cards = miniCardsHeight();
  assert.ok(
    cards <= MINI_HEIGHT,
    `cards view ${cards} px > window ${MINI_HEIGHT} px`,
  );
  assert.ok(MINI_HEIGHT - cards <= 8, "no large empty band under the cards");
  assert.equal(miniProcsHeight(), 2 + 8 + MINI.procHead + 5 * MINI.procRow);
  assert.match(miniStyle(), /--mini-procs:169px/);
  // Exactly MINI.procRows process rows whatever the data (0..12 processes).
  for (let n = 0; n <= 12; n++) {
    const rows = fixedRows(
      Array.from({ length: n }, (_, pid) => ({ pid })),
      MINI.procRows,
    );
    assert.equal(rows.length, MINI.procRows);
    assert.equal(rows.filter(Boolean).length, Math.min(n, MINI.procRows));
  }
  // 1080p at 150 %: 720 logical px minus a 48 px taskbar and the 8 px gap.
  assert.ok(MINI_HEIGHT + 8 <= 1080 / 1.5 - 48, "fits a 1080p screen at 150 %");
  // No frontend code can resize or move the mini window: view changes, data
  // refreshes and clicks (a blank-area click used to re-measure and call
  // fit_companion, which moved the whole window) only change the page.
  const files = [];
  const walk = (/** @type {string} */ dir) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (/\.(svelte|ts|js)$/.test(name)) files.push(path);
    }
  };
  walk("src");
  for (const path of files) {
    const text = readFileSync(path, "utf8");
    for (const banned of [
      "fit_companion",
      "setSize(",
      "setPosition(",
      "scrollHeight",
    ])
      assert.ok(!text.includes(banned), `${path} uses ${banned}`);
  }
  const mini = readFileSync(
    "src/lib/components/MenuBarOverview.svelte",
    "utf8",
  );
  assert.ok(
    !/class="mini"[^>]*on:click/.test(mini),
    "no click handler on the panel",
  );
  assert.ok(
    !mini.includes("data-tauri-drag-region"),
    "the mini window is not draggable",
  );
  console.log(
    "PASS: mini window has one fixed size; cards fit; clicks never resize or move it",
  );
}

{
  // Same machines as headline_temps in src-tauri/src/sampling.rs.
  const { cases } = JSON.parse(
    readFileSync(new URL("./fixtures/headline-temps.json", import.meta.url), "utf8"),
  );
  const key = (s) => (s == null ? null : s.id || s.value);
  for (const c of cases) {
    const sum = sensorSummary(c.sensors);
    assert.equal(key(sum.cpuTemp), c.cpu, `${c.machine}: CPU`);
    assert.equal(key(sum.gpuTemp), c.gpu, `${c.machine}: GPU`);
  }
  assert.equal(
    sensorDevice({ hardware: "AMD Ryzen 7 8845H w/ Radeon 780M Graphics", id: "/amdcpu/0/temperature/2" }),
    "cpu",
    "an APU is a CPU whatever its name says",
  );
  assert.equal(sensorDevice({ hardware: "Nuvoton", id: "/lpc/example/0/temperature/0" }), "other");
  assert.deepEqual(
    ["Core #1", "Core Average", "CCDs Max (Tdie)", "Tdie", "Core (Tctl/Tdie)", "CPU Package"].map(cpuTempRank),
    [5, 4, 3, 2, 1, 0],
  );
  console.log("PASS: CPU and GPU temperature picked per machine (Intel, AMD desktop, AMD APU laptop)");
}

{
  const { SERIES_COLOR, DASHED, sparkAreaPath } = await import("../src/lib/utils/chart-style.js");
  const src = (p) => readFileSync(new URL(`../src/${p}`, import.meta.url), "utf8");
  assert.equal(
    sparkAreaPath("M0,10L5,4M8,2L9,3", 36),
    "M0,10L5,4L5,36L0,36ZM8,2L9,3L9,36L8,36Z",
    "each run of the line closes to the baseline",
  );
  assert.equal(SERIES_COLOR.received, "var(--chart-1)");
  assert.ok(DASHED.has("sent") && DASHED.has("disk_write") && !DASHED.has("received"));
  for (const p of [
    "lib/components/stats/ResourceTrend.svelte",
    "lib/components/dashboard/Dashboard.svelte",
    "lib/components/dashboard/CardDetail.svelte",
  ]) {
    const s = src(p);
    assert.ok(s.includes("chart-style.js"), `${p} draws with the shared chart style`);
    assert.ok(!/var\(--(peach|yellow|blue|green)\)"/.test(s), `${p}: no chart colour of its own`);
  }
  assert.ok(
    !/@media[^{]*\{\s*\.resource-grid/.test(src("app.css")),
    "the Processes cards keep one spacing at every window width",
  );
  const page = src("routes/+page.svelte");
  assert.ok(page.includes('= "processes";'), "Fob starts on the Processes tab");
  console.log("PASS: one chart style everywhere; Processes tab default; card spacing fixed");
}

{
  const field = readFileSync(new URL("../src/lib/components/HotkeyField.svelte", import.meta.url), "utf8");
  assert.ok(/class=\{`flex h-8 w-44 /.test(field), "every hotkey box has one fixed width");
  assert.ok(field.includes('<span class="size-8 shrink-0" aria-hidden="true"></span>'), "an unset field keeps the clear button's slot");
  console.log("PASS: hotkey fields line up set or unset");
}
