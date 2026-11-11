import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  DASHBOARD_CARDS,
  PROCESS_CARDS,
  moveNextTo,
  normalizeOrder,
  previewOrder,
} from "../src/lib/utils/layout-order.js";
import {
  actionsWidth,
  columnLayout,
  columnWidth,
  manualMax,
  manualMin,
  manualWidths,
  tableWidth,
} from "../src/lib/utils/table-layout.js";

const D = ["a", "b", "c", "d"];

test("a saved order keeps its known ids once and puts missing ones back by their default neighbour", () => {
  assert.deepEqual(normalizeOrder(undefined, D), D);
  assert.deepEqual(normalizeOrder([], D), D);
  assert.deepEqual(normalizeOrder(["c", "a"], D), ["c", "d", "a", "b"]);
  assert.deepEqual(normalizeOrder(["d", "x", "d", 3, "b"], D), [
    "a",
    "d",
    "b",
    "c",
  ]);
  assert.deepEqual(normalizeOrder("abcd", D), D);
  for (const saved of [
    ["b", "b", "q"],
    ["d", "c", "b", "a", "a"],
  ]) {
    const order = normalizeOrder(saved, D);
    assert.deepEqual([...order].sort(), D, `every id once for ${saved}`);
  }
});

test("moving next to a target", () => {
  assert.deepEqual(moveNextTo(D, "a", "c", true), ["b", "c", "a", "d"]);
  assert.deepEqual(moveNextTo(D, "d", "a", false), ["d", "a", "b", "c"]);
  assert.deepEqual(moveNextTo(D, "a", "a", true), D);
  assert.deepEqual(moveNextTo(D, "a", "x", true), D);
});

test("card ids match the ones the settings file accepts", () => {
  const rust = readFileSync(
    new URL("../src-tauri/src/preferences.rs", import.meta.url),
    "utf8",
  );
  const list = (name) =>
    JSON.parse(
      `[${rust
        .match(new RegExp(`${name}: \\[&str; \\d+\\] = \\[([^\\]]*)\\]`))[1]
        .trim()
        .replace(/,$/, "")}]`,
    );
  assert.deepEqual(list("DASHBOARD_CARDS"), DASHBOARD_CARDS);
  assert.deepEqual(list("PROCESS_CARDS"), PROCESS_CARDS);
});

test("every process column can be saved in columnOrder", () => {
  const defs = readFileSync(
    new URL("../src/lib/definitions/columns.ts", import.meta.url),
    "utf8",
  );
  const ids = [...defs.matchAll(/id: "(\w+)"/g)].map((m) => m[1]);
  const config = JSON.parse(
    readFileSync(
      new URL("../src/lib/definitions/default-config.json", import.meta.url),
      "utf8",
    ),
  );
  assert.equal(ids[0], "name");
  assert.deepEqual(
    [...ids].sort(),
    Object.keys(config.appearance.columnVisibility).sort(),
  );
  assert.deepEqual(config.behavior.columnOrder, []);
});

const cols = (ids, hidden = []) =>
  ids.map((id) => ({ id, visible: !hidden.includes(id) }));
const total = ({ widths, filler }) =>
  Object.values(widths).reduce((a, b) => a + b, actionsWidth + filler);

test("short columns keep their content width; spare room goes to the name, then stays empty", () => {
  const columns = cols(
    ["name", "pid", "cpu_usage", "memory_usage", "user", "ppid"],
    ["ppid"],
  );
  const own = {
    pid: columnWidth("pid"),
    cpu_usage: columnWidth("cpu_usage"),
    memory_usage: columnWidth("memory_usage"),
    user: columnWidth("user"),
  };
  const min = tableWidth(columns);
  assert.equal(min, actionsWidth + 180 + 72 + 80 + 96 + 130);
  // Narrow: everything at its own width, nothing empty, the region scrolls.
  assert.deepEqual(columnLayout(columns, false, min - 300), {
    widths: { name: 180, ...own },
    filler: 0,
  });
  // A little room: the name column takes it.
  const some = columnLayout(columns, false, min + 50);
  assert.deepEqual(some, { widths: { name: 230, ...own }, filler: 0 });
  // Wide: the name column stops at 460, the rest stays empty before actions.
  for (const region of [1280, 1920, 3840]) {
    const wide = columnLayout(columns, false, region);
    assert.deepEqual(wide.widths, { name: 460, ...own });
    assert.equal(total(wide), region, `fills ${region}`);
  }
});

test("long text columns grow in turn and tree view widens its own columns", () => {
  const columns = cols(["name", "pid", "command", "user"]);
  const at = (region) => columnLayout(columns, false, region).widths;
  const min = tableWidth(columns);
  assert.equal(at(min + 100).name, 280);
  assert.equal(at(min + 100).command, 200);
  assert.equal(at(min + 400).name, 460);
  assert.equal(at(min + 400).command, 320);
  assert.equal(at(4000).command, 520);
  const tree = columnLayout(cols(["name", "cpu_usage"]), true, 0).widths;
  assert.deepEqual(tree, { name: 280, cpu_usage: 105 });
  assert.equal(columnLayout(cols(["name"]), true, 4000).widths.name, 560);
});

test("a width set by hand wins for its column and leaves the others alone", () => {
  const columns = cols(["name", "pid", "status", "user"]);
  const auto = columnLayout(columns, false, 1920);
  const pid = columnLayout(columns, false, 1920, { pid: 200 });
  assert.equal(pid.widths.pid, 200);
  assert.equal(pid.widths.status, auto.widths.status);
  assert.equal(pid.widths.user, auto.widths.user);
  assert.equal(total(pid), 1920);
  // A hand-set name column no longer grows; the room stays empty.
  const name = columnLayout(columns, false, 1920, { name: 220 });
  assert.equal(name.widths.name, 220);
  assert.equal(total(name), 1920);
  // Wider than the window: nothing grows and the region scrolls.
  const wide = columnLayout(columns, false, 600, { user: 900 });
  assert.equal(wide.filler, 0);
  assert.equal(tableWidth(columns, false, { user: 900 }) > 600, true);
});

test("saved widths keep known ids with finite numbers, rounded and clamped", () => {
  const ids = ["name", "pid", "user", "command"];
  assert.deepEqual(
    manualWidths(
      {
        name: 300.6,
        pid: -20,
        command: 99999,
        user: "wide",
        unknown: 200,
        ppid: NaN,
      },
      ids,
    ),
    { name: 301, pid: manualMin, command: manualMax },
  );
  assert.deepEqual(manualWidths({ pid: Infinity, user: null }, ids), {});
  for (const bad of [undefined, null, 5, "x", [120]])
    assert.deepEqual(manualWidths(bad, ids), {});
});

test("a drag in progress shows its landing order and drops back without one", () => {
  assert.deepEqual(previewOrder(D, { id: "a", target: "c", after: true }), [
    "b",
    "c",
    "a",
    "d",
  ]);
  assert.deepEqual(previewOrder(D, null), D);
});
