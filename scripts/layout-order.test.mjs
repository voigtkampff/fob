import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  DASHBOARD_CARDS,
  PROCESS_CARDS,
  moveNextTo,
  normalizeOrder,
  stepOrder,
} from "../src/lib/utils/layout-order.js";
import {
  nameWidth,
  nameMaxWidth,
  nameMinWidth,
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

test("moving next to a target and stepping skip hidden ids", () => {
  assert.deepEqual(moveNextTo(D, "a", "c", true), ["b", "c", "a", "d"]);
  assert.deepEqual(moveNextTo(D, "d", "a", false), ["d", "a", "b", "c"]);
  assert.deepEqual(moveNextTo(D, "a", "a", true), D);
  assert.deepEqual(moveNextTo(D, "a", "x", true), D);
  assert.deepEqual(stepOrder(D, "a", 1, ["a", "c", "d"]), ["b", "c", "a", "d"]);
  assert.deepEqual(stepOrder(D, "d", 1), D);
  assert.deepEqual(stepOrder(D, "a", -1), D);
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

test("the name column stays between its narrowest and widest, and the table still scrolls when the window is too narrow", () => {
  const columns = ["name", "pid", "cpu_usage", "memory_usage"].map((id) => ({
    id,
    visible: true,
  }));
  for (const tree of [false, true]) {
    const min = tableWidth(columns, tree);
    assert.equal(nameWidth(columns, tree, 0), nameMinWidth(tree));
    assert.equal(nameWidth(columns, tree, min - 50), nameMinWidth(tree));
    assert.equal(nameWidth(columns, tree, min + 40), nameMinWidth(tree) + 40);
    assert.equal(nameWidth(columns, tree, 3840), nameMaxWidth(tree));
  }
});
