import assert from "node:assert/strict";
import test from "node:test";
import { cpuGroups, groupPercent } from "../src/lib/utils/cpu-groups.js";
import { averageCpu } from "../src/lib/utils/dashboard.js";
import { translate } from "../src/lib/i18n/strings.js";

const range = (a, b) => Array.from({ length: b - a }, (_, i) => a + i);
/** example hybrid CPU as the backend reports it: 8 P-cores on threads 0-15, 8 E-cores on 16-23. */
const intel = (vendor = "intel") => ({
  state: "hybrid",
  vendor,
  groups: [
    { kind: "performance", cores: 8, threads: range(0, 16) },
    { kind: "efficiency", cores: 8, threads: range(16, 24) },
  ],
});
const names = (groups, lang) => groups.map((g) => translate(lang, g.label));

test("an Intel hybrid CPU shows P-cores and E-cores with their own load", () => {
  // A game on the P-cores, background work on the E-cores.
  const usage = [...Array(16).fill(80), ...Array(8).fill(10)];
  const groups = cpuGroups(intel(), usage);
  assert.deepEqual(names(groups, "zh"), ["P 核", "E 核"]);
  assert.deepEqual(names(groups, "en"), ["P-cores", "E-cores"]);
  assert.deepEqual(
    groups.map((g) => groupPercent(g.usage)),
    ["80", "10"],
  );
  assert.deepEqual(
    groups.map((g) => [g.cores, g.threads.length]),
    [
      [8, 16],
      [8, 8],
    ],
  );
});

test("an AMD hybrid CPU uses performance / efficiency names, never P / E", () => {
  const groups = cpuGroups(intel("amd"), Array(24).fill(5));
  assert.deepEqual(names(groups, "zh"), ["性能核", "能效核"]);
  assert.deepEqual(names(groups, "en"), [
    "Performance cores",
    "Efficiency cores",
  ]);
  assert.deepEqual(names(cpuGroups(intel("other"), Array(24).fill(5)), "zh"), [
    "性能核",
    "能效核",
  ]);
});

test("uniform and unread layouts show no split at all", () => {
  const usage = Array(16).fill(30);
  assert.equal(cpuGroups({ state: "uniform" }, usage), null);
  assert.equal(cpuGroups({ state: "unknown" }, usage), null);
  assert.equal(cpuGroups(undefined, usage), null);
  // A layout that does not cover exactly the threads on screen.
  assert.equal(cpuGroups(intel(), Array(20).fill(30)), null);
  assert.equal(cpuGroups(intel(), Array(32).fill(30)), null);
  const overlap = intel();
  overlap.groups[1].threads = range(15, 23);
  assert.equal(cpuGroups(overlap, Array(24).fill(30)), null);
});

test("a P-core busy on one of its two threads counts as half busy, like the total", () => {
  // Every P-core runs one thread at 100%, its sibling idle; E-cores idle.
  const usage = [
    ...range(0, 16).map((i) => (i % 2 ? 0 : 100)),
    ...Array(8).fill(0),
  ];
  const [p, e] = cpuGroups(intel(), usage);
  assert.equal(p.usage, 50, "average of the 16 P threads, not 8 cores at 100%");
  assert.equal(e.usage, 0, "an idle group is a real 0%");
  // Groups are averages, never a sum over 100%, and weighting them by their
  // threads gives back the whole-CPU figure.
  const all = cpuGroups(intel(), Array(24).fill(100));
  assert.ok(all.every((g) => g.usage === 100));
  const weighted = (p.usage * 16 + e.usage * 8) / 24;
  assert.equal(weighted, averageCpu(usage));
});

test("a missing thread reading shows a dash, not 0%", () => {
  const usage = [...Array(16).fill(20), NaN, ...Array(7).fill(0)];
  const [p, e] = cpuGroups(intel(), usage);
  assert.equal(groupPercent(p.usage), "20");
  assert.equal(e.usage, null);
  assert.equal(groupPercent(e.usage), "—");
});
