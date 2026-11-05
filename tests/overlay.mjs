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
  dropSlot,
  inkShift,
  overlayOpacity,
  chartFor,
  OPACITY_FLOOR,
  CHART_METRICS,
  chartMetrics,
  chartsPatch,
  toggleChart,
  overlayTheme,
  overlayThemeOptions,
  overlayColors,
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
    locked: false,
    charts: [],
    theme: "",
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
// 0.7.3 per-metric charts; the 0.7.1 flag migrates to all or none.
assert.deepEqual(CHART_METRICS, [
  "cpu",
  "cpuTemp",
  "gpu",
  "gpuTemp",
  "ram",
  "disk",
  "net",
]);
assert.deepEqual(
  overlaySettings({ overlayCharts: true }).charts,
  CHART_METRICS,
);
assert.deepEqual(overlaySettings({ overlayCharts: false }).charts, []);
assert.deepEqual(overlaySettings({ overlayCharts: "yes" }).charts, []);
assert.deepEqual(overlaySettings({}).charts, []);
assert.deepEqual(
  chartMetrics({
    overlayCharts: true,
    overlayChartMetrics: ["net", "vram", "cpu"],
  }),
  ["cpu", "net"],
  "the list wins over the old flag; no chart for video memory; fixed order",
);
assert.deepEqual(
  chartMetrics({ overlayCharts: true, overlayChartMetrics: [] }),
  [],
);
assert.deepEqual(toggleChart(["net"], "cpu", true), ["cpu", "net"]);
assert.deepEqual(toggleChart(["cpu", "net"], "cpu", false), ["net"]);
assert.deepEqual(
  toggleChart(["cpu"], "vram", true),
  ["cpu"],
  "no history: ignored",
);
assert.deepEqual(chartsPatch(["net", "cpu"]), {
  overlayChartMetrics: ["cpu", "net"],
});
assert.deepEqual(chartsPatch([]), { overlayChartMetrics: [] });
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
// 0.7.5 own appearance: unknown/removed/unset/glassy → follow the main theme.
const TH = {
  dracula: { name: "dracula", label: "Dracula" },
  nord: { name: "nord", label: "Nord" },
  glassy: { name: "glassy", label: "Glassy" },
};
const main = TH.nord;
assert.equal(overlayTheme("", main, TH), main);
assert.equal(overlayTheme(undefined, main, TH), main);
assert.equal(overlayTheme(42, main, TH), main);
assert.equal(overlayTheme("dracula", main, TH), TH.dracula);
assert.equal(overlayTheme("removedTheme", main, TH), main);
assert.equal(overlayTheme("toString", main, TH), main, "no prototype keys");
assert.equal(overlayTheme("glassy", main, TH), main);
assert.deepEqual(overlayThemeOptions(TH), [
  { value: "dracula", label: "Dracula" },
  { value: "nord", label: "Nord" },
]);
assert.equal(overlaySettings({ overlayTheme: "nord" }).theme, "nord");
assert.equal(overlaySettings({ overlayTheme: 3 }).theme, "");
assert.deepEqual(
  overlayColors({
    background: "#000",
    foreground: "#fff",
    primary: "#0af",
    border: "#333",
    "chart-1": "#f0a",
  }),
  {
    "--ob-bg": "#000",
    "--ob-fg": "#fff",
    "--ob-accent": "#0af",
    "--ob-border": "#333",
    "--ob-chart": "#f0a",
  },
);
// Pointer drag: slot from the pointer's y over the shown rows' boxes.
const rowsY = [0, 28, 56, 84].map((top) => ({ top, bottom: top + 28 }));
assert.equal(dropSlot(-10, rowsY), 0);
assert.equal(dropSlot(13, rowsY), 0);
assert.equal(dropSlot(15, rowsY), 1);
assert.equal(dropSlot(70, rowsY), 3);
assert.equal(dropSlot(99, rowsY), 4);
assert.equal(dropSlot(500, rowsY), 4);
assert.equal(dropSlot(5, []), 0);
// Dragging CPU (row 0) below the last row puts it last.
assert.deepEqual(reorderMetric(four, 0, dropSlot(120, rowsY)), [
  "gpu",
  "ram",
  "net",
  "cpu",
]);
// Ink centring from font metrics (11px; YaHei UI-like tall metrics vs a
// font whose ascent+descent equals the size).
const close = (a, b) => assert.ok(Math.abs(a - b) < 1e-9, `${a} ≈ ${b}`);
close(
  inkShift(10, {
    fontBoundingBoxAscent: 8,
    fontBoundingBoxDescent: 2,
    actualBoundingBoxAscent: 6,
    actualBoundingBoxDescent: 0,
  }),
  0,
);
// Ideographs (ink 9.5 above / 1.2 below baseline) in a font with 11.6 / 3.0
// metrics: baseline at 9.8, ink centre 5.65, so 0.15 px up.
close(
  inkShift(11, {
    fontBoundingBoxAscent: 11.6,
    fontBoundingBoxDescent: 3,
    actualBoundingBoxAscent: 9.5,
    actualBoundingBoxDescent: 1.2,
  }),
  -0.15,
);
// Digits sit on the baseline with no descent; in a 13px box whose metrics
// are 13 / 3, the baseline is at 11.5 and the ink centre at 6.85: 0.35 px up.
close(
  inkShift(13, {
    fontBoundingBoxAscent: 13,
    fontBoundingBoxDescent: 3,
    actualBoundingBoxAscent: 9.3,
    actualBoundingBoxDescent: 0,
  }),
  -0.35,
);
console.log("PASS: status bar settings, metric order and cell text");
