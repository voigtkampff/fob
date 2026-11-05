import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  METRICS,
  DEFAULT_METRICS,
  overlaySettings,
  toggleMetric,
  moveMetric,
  settingsRows,
  shortRate,
  cells,
  reorderMetric,
  overlayOpacity,
  chartFor,
  OPACITY_FLOOR,
} from "../src/lib/utils/overlay.js";

const defaults = JSON.parse(
  readFileSync(
    new URL("../src/lib/definitions/default-config.json", import.meta.url),
  ),
).behavior;
assert.deepEqual(
  overlaySettings(defaults),
  {
    enabled: false,
    metrics: DEFAULT_METRICS,
    dock: "top",
    opacity: 60,
    unlocked: false,
    charts: false,
  },
  "off by default; the saved defaults match the code's",
);
const rust = readFileSync(
  new URL("../src-tauri/src/preferences.rs", import.meta.url),
  "utf8",
);
for (const id of METRICS)
  assert.ok(rust.includes(`"${id}"`), `preferences.rs accepts ${id}`);
assert.ok(!METRICS.includes("fps"), "no FPS (it would need injection)");

const bad = overlaySettings({
  overlayEnabled: "yes",
  overlayMetrics: ["net", "fps", "net", "cpu"],
  overlayDock: "left",
  overlayOpacity: 250,
});
assert.deepEqual(
  bad.metrics,
  ["net", "cpu"],
  "unknown and repeated ids dropped, order kept",
);
assert.equal(bad.enabled, false);
assert.equal(bad.dock, "top");
assert.equal(bad.opacity, 100);
assert.equal(overlaySettings({ overlayOpacity: -3 }).opacity, OPACITY_FLOOR);
// 0.7.1: whole-bar opacity, 10–100; 0.7.0 values under 10 move up to 10.
assert.equal(overlayOpacity(0), 10);
assert.equal(overlayOpacity(9.4), 10);
assert.equal(overlayOpacity(10), 10);
assert.equal(overlayOpacity(55.6), 56);
assert.equal(overlayOpacity(250), 100);
assert.equal(overlayOpacity(undefined), 60);
assert.equal(overlayOpacity("x"), 60);
assert.equal(overlaySettings({ overlayCharts: true }).charts, true);
assert.equal(overlaySettings({ overlayCharts: "yes" }).charts, false);
assert.deepEqual(
  overlaySettings({ overlayMetrics: [] }).metrics,
  [],
  "all off stays all off",
);

let shown = ["cpu", "gpu", "ram"];
shown = toggleMetric(shown, "net", true);
assert.deepEqual(shown, ["cpu", "gpu", "ram", "net"]);
shown = toggleMetric(shown, "gpu", false);
assert.deepEqual(shown, ["cpu", "ram", "net"]);
assert.deepEqual(moveMetric(shown, "net", -1), ["cpu", "net", "ram"]);
assert.deepEqual(moveMetric(shown, "cpu", -1), shown, "first can't move up");
assert.deepEqual(moveMetric(shown, "net", 1), shown, "last can't move down");
const rows = settingsRows(["net", "cpu"]);
assert.deepEqual(rows.slice(0, 2), [
  { id: "net", on: true },
  { id: "cpu", on: true },
]);
assert.equal(rows.length, METRICS.length);

assert.equal(shortRate(0), "0B");
assert.equal(shortRate(35 * 1024), "35K");
assert.equal(shortRate(1.25 * 1024 * 1024), "1.3M");
assert.equal(shortRate(null), "—");
assert.deepEqual(
  cells(
    {
      cpu: 12.4,
      cpuTemp: 55.6,
      ramUsed: 13.4 * 1024 ** 3,
      netRx: 2048,
      netTx: 0,
    },
    ["cpu", "cpuTemp", "ram", "net", "gpu"],
  ),
  [
    { id: "cpu", value: "12%", parts: ["12%"] },
    { id: "cpuTemp", value: "56°C", parts: ["56°C"] },
    { id: "ram", value: "13.4G", parts: ["13.4G"] },
    { id: "net", value: "↓2.0K ↑0B", parts: ["↓2.0K", "↑0B"] },
    { id: "gpu", value: "—", parts: ["—"] },
  ],
);
assert.deepEqual(cells({ diskRead: 0, diskWrite: 5000 }, ["disk"])[0].parts, [
  "R 0B",
  "W 4.9K",
]);
// Drag and drop: `to` is the slot before removal (0..length).
const four = ["cpu", "gpu", "ram", "net"];
assert.deepEqual(reorderMetric(four, 0, 4), ["gpu", "ram", "net", "cpu"]);
assert.deepEqual(reorderMetric(four, 3, 0), ["net", "cpu", "gpu", "ram"]);
assert.deepEqual(reorderMetric(four, 1, 3), ["cpu", "ram", "gpu", "net"]);
assert.equal(reorderMetric(four, 1, 1), four, "dropped on itself");
assert.equal(reorderMetric(four, 1, 2), four, "dropped right after itself");
assert.equal(reorderMetric(four, -1, 2), four);
assert.equal(reorderMetric(four, 0, 9), four);
assert.deepEqual(four, ["cpu", "gpu", "ram", "net"], "input untouched");
// Charts: percent metrics scale to 100, rates to their peak (≥ 1 KB/s),
// and video memory / power have no history.
const hist = [
  { timestamp: 1, cpu: 10, gpus: { a: 20, b: 70 }, received: 3000, sent: 1000 },
  { timestamp: 2, cpu: null, gpus: {}, received: null, sent: null },
];
assert.equal(chartFor("cpu", hist).max, 100);
assert.equal(chartFor("gpu", hist).pick(hist[0]), 70);
assert.equal(chartFor("gpu", hist).pick(hist[1]), null);
assert.equal(chartFor("net", hist).max, 4000);
assert.equal(chartFor("net", hist).pick(hist[1]), null);
assert.equal(chartFor("disk", []).max, 1024);
assert.equal(chartFor("vram", hist), null);
assert.equal(chartFor("power", hist), null);
console.log("PASS: status bar settings, metric order and cell text");
