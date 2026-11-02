import assert from "node:assert/strict";
import {
  formatRate,
  formatGB,
  formatPercent,
  formatUptimeShort,
  averageCpu,
  systemVolume,
  gpuSummary,
  topByMemory,
  series,
  lastN,
  chartMax,
  sparklinePath,
} from "../src/lib/utils/dashboard.js";

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

assert.equal(averageCpu([10, 20, 30, 40]), 25);
assert.equal(averageCpu([]), null);
assert.equal(averageCpu([10, NaN]), null);

const vols = [
  { mount: "D:\\", total: 2000, available: 1000 },
  { mount: "C:\\", total: 1000, available: 400 },
];
assert.equal(systemVolume(vols)?.mount, "C:\\");
assert.equal(systemVolume([{ mount: "/boot", total: 1, available: 1 }, { mount: "/", total: 9, available: 3 }])?.mount, "/");
assert.equal(systemVolume([{ mount: "/a", total: 1, available: 1 }, { mount: "/b", total: 9, available: 3 }])?.mount, "/b");
assert.equal(systemVolume([]), null);

assert.deepEqual(gpuSummary([]), { percent: null, name: null });
assert.deepEqual(gpuSummary([{ name: "iGPU", utilization: null }]), { percent: null, name: "iGPU" });
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
assert.deepEqual(topByMemory(procs, 3).map((p) => p.pid), [1, 2, 3]);
assert.equal(procs[0].pid, 3, "input is not mutated");

assert.deepEqual(series([{ cpu: 1 }, { cpu: null }, { cpu: NaN }, {}], "cpu"), [1, null, null, null]);
assert.deepEqual(lastN([1, 2, 3, 4], 2), [3, 4]);
assert.deepEqual(lastN([1], 2), [1]);
assert.equal(chartMax([[null, 10], [20]]), 1024);
assert.equal(chartMax([[5000], [null]]), 5000);

assert.equal(sparklinePath([], { width: 10, height: 10, max: 1 }), "");
assert.equal(sparklinePath([0, 50, 100], { width: 100, height: 10, max: 100 }), "M0.0,10.0L50.0,5.0L100.0,0.0");
assert.equal(sparklinePath([10, null, 10], { width: 2, height: 10, max: 10 }), "M0.0,0.0M2.0,0.0");
// fewer points than slots are right-aligned; values above max are clamped
assert.equal(sparklinePath([200], { width: 100, height: 10, max: 100, slots: 3 }), "M100.0,0.0");

console.log("PASS: dashboard formatting, volume/GPU selection, top processes, sparkline series");
