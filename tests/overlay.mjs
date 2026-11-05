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
assert.equal(overlaySettings({ overlayOpacity: -3 }).opacity, 0);
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
    { id: "cpu", value: "12%" },
    { id: "cpuTemp", value: "56°C" },
    { id: "ram", value: "13.4G" },
    { id: "net", value: "↓2.0K ↑0B" },
    { id: "gpu", value: "—" },
  ],
);
console.log("PASS: status bar settings, metric order and cell text");
