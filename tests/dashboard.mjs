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
  lastN,
  chartMax,
  sparklinePath,
  topBy,
  sensorSummary,
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

assert.deepEqual(formatRate(null), { value: "—", unit: "" });
assert.deepEqual(formatRate(-1), { value: "—", unit: "" });
assert.deepEqual(formatRate(0), { value: "0", unit: "B/s" });
assert.deepEqual(formatRate(512), { value: "512", unit: "B/s" });
assert.deepEqual(formatRate(437 * 1024), { value: "437", unit: "KB/s" });
assert.deepEqual(formatRate(4.9 * 1024 * 1024), { value: "4.9", unit: "MB/s" });
assert.equal(formatGB(24.96 * 1024 ** 3, 2), "24.96 GB");
assert.equal(formatPercent(null), "n/a");
assert.equal(formatPercent(51.6), "52");

assert.equal(formatUptimeShort(26 * 3600 + 5), "up 1 day, 2 hr");
assert.equal(formatUptimeShort(3 * 86400), "up 3 days, 0 hr");
assert.equal(formatUptimeShort(3 * 3600 + 4 * 60), "up 3 hr, 4 min");
assert.equal(formatUptimeShort(59), "up 0 min");
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
assert.deepEqual(lastN([1, 2, 3, 4], 2), [3, 4]);
assert.deepEqual(lastN([1], 2), [1]);
assert.equal(chartMax([[null, 10], [20]]), 1024);
assert.equal(chartMax([[5000], [null]]), 5000);

assert.equal(sparklinePath([], { width: 10, height: 10, max: 1 }), "");
assert.equal(
  sparklinePath([0, 50, 100], { width: 100, height: 10, max: 100 }),
  "M0.0,10.0L50.0,5.0L100.0,0.0",
);
assert.equal(
  sparklinePath([10, null, 10], { width: 2, height: 10, max: 10 }),
  "M0.0,0.0M2.0,0.0",
);
// fewer points than slots are right-aligned; values above max are clamped
assert.equal(
  sparklinePath([200], { width: 100, height: 10, max: 100, slots: 3 }),
  "M100.0,0.0",
);

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
    legacy: false,
    fake: false,
  };
  assert.equal(setupState(null), "none");
  // winperf 0.4 still installed: offer the move to Fob first, whatever else.
  assert.equal(setupState({ ...base, legacy: true }), "migrate");
  assert.equal(
    setupState({
      ...base,
      legacy: true,
      installed: true,
      task: "ok",
      running: true,
    }),
    "migrate",
  );
  assert.equal(
    setupState({ ...base, legacy: true, bundled: false }),
    "missing-files",
  );
  const moving = setupSteps({ ...base, legacy: true, pawnio_installed: true });
  assert.equal(moving.length, 3);
  assert.match(
    moving[0],
    /winperf-sensors.*C:\\Program Files\\winperf\\sensors/,
  );
  assert.match(moving[1], /C:\\Program Files\\Fob\\sensors/);
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
