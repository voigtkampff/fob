import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  METRICS,
  DEFAULT_METRICS,
  BLOCK_IDS,
  DEFAULT_BLOCKS,
  DEFAULT_FIELD_ORDER,
  overlayVisibleBlocks,
  overlayBlockOrder,
  overlayFieldOrder,
  overlaySettings,
  settingsBlocks,
  toggleBlockVisibility,
  toggleMetric,
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
    order: BLOCK_IDS,
    blocks: DEFAULT_BLOCKS,
    fieldOrder: {
      cpu: ["cpu", "cpuPower", "cpuTemp", "cpuVoltage", "cpuClock"],
      gpu: ["gpu", "power", "gpuTemp"],
    },
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
  ["cpu", "net"],
  "unknown and repeated ids dropped, fields in their canonical order",
);
assert.deepEqual(
  bad.order,
  ["net", "cpu", "gpu", "ram", "vram", "disk"],
  "no saved block order: the old field order decides, missing blocks follow",
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
assert.deepEqual(CHART_METRICS, ["cpu", "gpu", "ram", "disk", "net"]);
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
// 0.8.13: a block order carries whole families; fields never have a slot.
assert.deepEqual(
  overlayBlockOrder(undefined, ["net", "gpuTemp", "cpu"]),
  ["net", "gpu", "cpu", "ram", "vram", "disk"],
  "old flat order gives the first blocks their old place, then the rest",
);
assert.deepEqual(
  overlayBlockOrder(["net", "bogus", "gpu", "net"], ["cpu"]),
  ["cpu", "net", "gpu", "ram", "vram", "disk"],
  "saved order: unknown/repeated dropped, missing back in default place",
);
// Parent visibility is separate from the child switches.
const childFields = ["cpu", "cpuTemp", "cpuVoltage"];
const onBlocks = ["cpu"];
const shownBlock = settingsBlocks(
  childFields,
  BLOCK_IDS,
  [],
  DEFAULT_FIELD_ORDER,
  {},
  onBlocks,
)[0];
const hiddenBlock = settingsBlocks(
  childFields,
  BLOCK_IDS,
  [],
  DEFAULT_FIELD_ORDER,
  {},
  toggleBlockVisibility(onBlocks, "cpu", false),
)[0];
assert.equal(hiddenBlock.on, false);
assert.deepEqual(
  hiddenBlock.rows.map((r) => [r.id, r.on]),
  shownBlock.rows.map((r) => [r.id, r.on]),
  "parent off keeps every child switch",
);
assert.deepEqual(
  settingsBlocks(
    childFields,
    BLOCK_IDS,
    [],
    DEFAULT_FIELD_ORDER,
    {},
    toggleBlockVisibility([], "cpu", true),
  )[0].rows.map((r) => [r.id, r.on]),
  shownBlock.rows.map((r) => [r.id, r.on]),
  "parent on restores the same child switches",
);
assert.deepEqual(
  toggleBlockVisibility(["gpu"], "cpu", true),
  ["cpu", "gpu"],
  "parent visibility is a set, not a field list",
);
// A field off/on keeps its slot; dragging reorders without touching switches.
const fieldOrder = {
  ...DEFAULT_FIELD_ORDER,
  cpu: ["cpuPower", "cpu", "cpuTemp", "cpuVoltage", "cpuClock"],
};
const beforeField = settingsBlocks(
  childFields,
  BLOCK_IDS,
  ["cpu"],
  fieldOrder,
  {},
  onBlocks,
)[0];
const withoutTemp = settingsBlocks(
  childFields.filter((id) => id !== "cpuTemp"),
  BLOCK_IDS,
  ["cpu"],
  fieldOrder,
  {},
  onBlocks,
)[0];
assert.deepEqual(
  withoutTemp.rows.map((r) => r.id),
  beforeField.rows.map((r) => r.id),
  "a disabled field keeps its position",
);
const dragged = settingsBlocks(
  childFields,
  BLOCK_IDS,
  ["cpu"],
  fieldOrder,
  { cpu: { id: "cpuPower", target: "cpu", after: true } },
  onBlocks,
)[0];
assert.deepEqual(
  dragged.rows.map((r) => [r.id, r.on]).sort(),
  beforeField.rows.map((r) => [r.id, r.on]).sort(),
  "dragging only changes order, never a switch",
);
assert.deepEqual(
  overlayVisibleBlocks(undefined, ["cpu", "cpuTemp", "gpu"]),
  ["cpu", "gpu"],
  "old configs make a block visible when any field was on",
);
const blockDragged = settingsBlocks(
  childFields,
  ["gpu", ...BLOCK_IDS.filter((id) => id !== "gpu")],
  [],
  DEFAULT_FIELD_ORDER,
  {},
  onBlocks,
);
assert.deepEqual(
  blockDragged.flatMap((b) => b.rows.map((r) => [b.id, r.id, r.on])).sort(),
  settingsBlocks(childFields, BLOCK_IDS, [], DEFAULT_FIELD_ORDER, {}, onBlocks)
    .flatMap((b) => b.rows.map((r) => [b.id, r.id, r.on]))
    .sort(),
  "dragging a block changes no switch",
);
// Order and enabled state are independent: unchecking a block leaves it in
// place (and greyed), and checking it again returns to the same slot.
const fullOrder = ["net", "gpu", "cpu", "ram", "vram", "disk"];
let orderFields = ["net", "gpu", "cpu"];
const off = toggleMetric(orderFields, "gpu", false);
const offBlocks = settingsBlocks(
  off,
  fullOrder,
  [],
  DEFAULT_FIELD_ORDER,
  {},
  DEFAULT_BLOCKS,
);
assert.equal(
  offBlocks.findIndex((b) => b.id === "gpu"),
  1,
  "an unchecked block keeps its position",
);
assert.equal(
  offBlocks.find((b) => b.id === "gpu").on,
  true,
  "a field off does not hide its parent block",
);
assert.equal(
  offBlocks.find((b) => b.id === "gpu").rows.find((r) => r.id === "gpu").on,
  false,
  "the field switch itself is off",
);
const back = toggleMetric(off, "gpu", true);
assert.deepEqual(overlayBlockOrder(fullOrder, back), fullOrder);
assert.equal(
  settingsBlocks(
    back,
    fullOrder,
    [],
    DEFAULT_FIELD_ORDER,
    {},
    DEFAULT_BLOCKS,
  ).findIndex((b) => b.id === "gpu"),
  1,
  "checking it again uses the same position",
);
const sections = settingsBlocks(
  ["cpu", "cpuPower", "cpuTemp"],
  ["cpu", "gpu", "ram", "vram", "disk", "net"],
  ["cpu"],
  DEFAULT_FIELD_ORDER,
  {},
  DEFAULT_BLOCKS,
);
assert.deepEqual(
  sections.map((b) => [b.id, b.single, b.on, b.chartable]),
  [
    ["cpu", false, true, true],
    ["gpu", false, true, true],
    ["ram", true, true, true],
    ["vram", true, false, false],
    ["disk", true, false, true],
    ["net", true, true, true],
  ],
);
assert.deepEqual(
  sections[0].rows.map((r) => [r.id, r.on]),
  [
    ["cpu", true],
    ["cpuPower", true],
    ["cpuTemp", true],
    ["cpuVoltage", false],
    ["cpuClock", false],
  ],
  "CPU slots: usage, power, temperature, voltage, frequency",
);
assert.deepEqual(
  sections[1].rows.map((r) => [r.id, r.on]),
  [
    ["gpu", false],
    ["power", false],
    ["gpuTemp", false],
  ],
  "GPU slots: usage, power, temperature",
);
assert.deepEqual(sections[2].rows, [], "single blocks have no children");
assert.equal(sections[2].field, "ram");

assert.deepEqual(overlayFieldOrder(undefined), DEFAULT_FIELD_ORDER);
assert.deepEqual(
  overlayFieldOrder({
    cpu: ["cpuPower", "cpu", "cpuTemp"],
    gpu: ["power", "gpu"],
  }),
  {
    cpu: ["cpuPower", "cpu", "cpuTemp", "cpuVoltage", "cpuClock"],
    gpu: ["power", "gpu", "gpuTemp"],
  },
  "a saved order is kept; missing slots follow in default order",
);
assert.deepEqual(
  overlayFieldOrder({ cpu: ["cpu", "cpu", "bogus"], gpu: [] }),
  DEFAULT_FIELD_ORDER,
  "repeated/unknown slots fall back to the default order",
);

// Every settings icon must carry a native title, including blocked rows;
// aria-disabled keeps Chromium's title working where disabled would not.
const settingsSource = readFileSync(
  new URL("../src/lib/components/SettingsWindow.svelte", import.meta.url),
  "utf8",
);
assert.ok(!settingsSource.includes("native-tip"), "no custom tooltip CSS");
assert.ok(!settingsSource.includes("showTitle"), "no custom tooltip timer");
assert.ok(!settingsSource.includes("clearTitle"), "no custom tooltip timer");
const settingsLines = settingsSource.split("\n");
const iconLines = settingsLines
  .map((line, i) => (line.includes('class="icon-toggle"') ? i : -1))
  .filter((i) => i >= 0);
assert.ok(iconLines.length >= 3, "settings has icon switches");
assert.ok(
  settingsLines.filter((line) => line.includes("<Tooltip.Root>")).length >=
    iconLines.length,
  "every icon is inside a Tooltip.Root",
);
for (const at of iconLines) {
  const around = settingsLines.slice(Math.max(0, at - 14), at + 40).join("\n");
  assert.match(around, /<Tooltip\.Trigger>/, "icon wrapped in Tooltip.Trigger");
  assert.match(around, /<Tooltip\.Content/, "icon has Tooltip.Content");
  assert.doesNotMatch(
    around,
    /\n\s*disabled=\{/,
    "tooltip triggers must not be disabled",
  );
}

const head = (id, label) => ({
  id,
  key: `${id}:block`,
  block: id,
  value: "",
  parts: [],
  label,
});

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
    BLOCK_IDS,
  ),
  [
    head("cpu", "CPU"),
    { id: "cpu", value: "12%", parts: ["12%"], label: "" },
    { id: "cpuTemp", value: "56°C", parts: ["56°C"], label: "" },
    head("gpu", "GPU"),
    { id: "gpu", value: "—", parts: ["—"], label: "" },
    { id: "ram", value: "13.4G", parts: ["13.4G"], label: null },
    { id: "net", value: "↓2.0K ↑0B", parts: ["↓2.0K", "↑0B"], label: null },
  ],
);
// The final bar layout: each slot is its own cell, in the family order.
assert.deepEqual(
  cells(
    {
      cpu: 15,
      cpuPower: 45,
      cpuTemp: 47,
      cpuVoltage: 1.28,
      cpuVoltageSource: "measured",
      cpuClock: 5320,
      gpu: 80,
      gpuPower: 180,
      gpuTemp: 62,
    },
    [
      "cpu",
      "cpuPower",
      "gpu",
      "power",
      "cpuVoltage",
      "cpuClock",
      "cpuTemp",
      "gpuTemp",
    ],
    ["cpu", "gpu"],
    DEFAULT_FIELD_ORDER,
  ),
  [
    head("cpu", "CPU"),
    { id: "cpu", value: "15%", parts: ["15%"], label: "" },
    { id: "cpuPower", value: "45W", parts: ["45W"], label: "" },
    { id: "cpuTemp", value: "47°C", parts: ["47°C"], label: "" },
    { id: "cpuVoltage", value: "1.28V", parts: ["1.28V"], label: "Vcore" },
    { id: "cpuClock", value: "5.32GHz", parts: ["5.32GHz"], label: "CLK" },
    head("gpu", "GPU"),
    { id: "gpu", value: "80%", parts: ["80%"], label: "" },
    { id: "power", value: "180W", parts: ["180W"], label: "" },
    { id: "gpuTemp", value: "62°C", parts: ["62°C"], label: "" },
  ],
  "CPU 15% 45W  47°C  Vcore 1.28V  CLK 5.32GHz | GPU 80% 180W  62°C",
);
// The block order moves whole families; the family field order moves slots.
assert.deepEqual(
  cells(
    { cpu: 15, cpuTemp: 47, gpu: 80 },
    ["cpu", "cpuTemp", "gpu"],
    ["gpu", "cpu"],
  )
    .filter((c) => !c.block)
    .map((c) => c.id),
  ["gpu", "cpu", "cpuTemp"],
);
assert.deepEqual(
  cells(
    { cpu: 15, cpuPower: 45, cpuTemp: 47, gpu: 80, gpuPower: 180 },
    ["cpu", "cpuPower", "cpuTemp", "gpu", "power"],
    ["cpu", "gpu"],
    {
      cpu: ["cpuPower", "cpu", "cpuTemp"],
      gpu: ["power", "gpu"],
    },
  )
    .filter((c) => !c.block)
    .map((c) => c.id),
  ["cpuPower", "cpu", "cpuTemp", "power", "gpu"],
);
// The block name is fixed first even when usage is moved to the end.
assert.deepEqual(
  cells(
    { cpu: 40, cpuPower: 63, cpuVoltage: 1.27 },
    ["cpu", "cpuPower", "cpuVoltage"],
    ["cpu"],
    {
      cpu: ["cpuVoltage", "cpuPower", "cpu"],
      gpu: DEFAULT_FIELD_ORDER.gpu,
    },
  )
    .filter((c) => !c.block)
    .map((c) => c.id),
  ["cpuVoltage", "cpuPower", "cpu"],
);
assert.equal(
  cells(
    { cpu: 40, cpuPower: 63, cpuVoltage: 1.27 },
    ["cpu", "cpuPower", "cpuVoltage"],
    ["cpu"],
    {
      cpu: ["cpuVoltage", "cpuPower", "cpu"],
      gpu: DEFAULT_FIELD_ORDER.gpu,
    },
  )[0].label,
  "CPU",
);
// THR follows CPU temperature, or the last CPU field when temp is off.
const badge = cells(
  { cpu: 100, cpuTemp: 98, cpuPower: 188 },
  ["cpu", "cpuTemp", "cpuPower"],
  ["cpu"],
  DEFAULT_FIELD_ORDER,
  ["cpu"],
  true,
);
assert.equal(
  badge.find((c) => c.id === "cpuTemp").throttleAfter,
  true,
  "THR follows the temperature cell",
);
const badgeNoTemp = cells(
  { cpu: 100, cpuPower: 188, cpuClock: 4610 },
  ["cpu", "cpuPower", "cpuClock"],
  ["cpu"],
  DEFAULT_FIELD_ORDER,
  ["cpu"],
  true,
);
assert.equal(
  badgeNoTemp.filter((c) => !c.block).at(-1).throttleAfter,
  true,
  "without temperature THR goes at the CPU block end",
);
// Power is an independent slot, with no name of its own.
assert.deepEqual(cells({ cpu: 15, cpuPower: 45 }, ["cpu", "cpuPower"]), [
  head("cpu", "CPU"),
  { id: "cpu", value: "15%", parts: ["15%"], label: "" },
  { id: "cpuPower", value: "45W", parts: ["45W"], label: "" },
]);
assert.deepEqual(cells({ cpuPower: 45 }, ["cpuPower"]), [
  head("cpu", "CPU"),
  { id: "cpuPower", value: "45W", parts: ["45W"], label: "" },
]);
assert.deepEqual(cells({ cpu: 15 }, ["cpu", "cpuPower"]), [
  head("cpu", "CPU"),
  { id: "cpu", value: "15%", parts: ["15%"], label: "" },
  { id: "cpuPower", value: "—", parts: ["—"], label: "" },
]);
assert.deepEqual(cells({}, ["cpu", "cpuPower"]), [
  head("cpu", "CPU"),
  { id: "cpu", value: "—", parts: ["—"], label: "" },
  { id: "cpuPower", value: "—", parts: ["—"], label: "" },
]);
// Drag and drop helpers still used elsewhere.
const four = ["cpu", "gpu", "ram", "net"];
assert.deepEqual(reorderMetric(four, 0, 4), ["gpu", "ram", "net", "cpu"]);
assert.deepEqual(reorderMetric(four, 3, 0), ["net", "cpu", "gpu", "ram"]);
assert.deepEqual(reorderMetric(four, 1, 3), ["cpu", "ram", "gpu", "net"]);
assert.equal(reorderMetric(four, 1, 1), four, "dropped on itself");
assert.equal(reorderMetric(four, 1, 2), four, "dropped right after itself");
assert.equal(reorderMetric(four, -1, 2), four);
assert.equal(reorderMetric(four, 0, 9), four);
assert.deepEqual(four, ["cpu", "gpu", "ram", "net"], "input untouched");
assert.equal(dropSlot(10, [{ top: 0, bottom: 20 }]), 1);
// Charts: usage and the single readings that had one scale; power, voltage
// and frequency have no curve at all.
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
assert.equal(chartFor("cpuPower", hist), null);
assert.equal(chartFor("cpuClock", hist), null);
assert.equal(chartFor("cpuVoltage", hist), null);
assert.equal(chartFor("cpuTemp", hist).max, 100);

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
