import assert from "node:assert/strict";
import test from "node:test";
import {
  menuPlacement,
  procMenuItems,
  procRowOf,
} from "../src/lib/utils/process-menu.js";

const ctx = {
  onShowDetails() {},
  onTogglePin() {},
  onKillProcess() {},
  copyText() {},
  onOpenLocation() {},
};
const ids = (items) => items.map((item) => item.id);
const item = (items, id) => items.find((entry) => entry.id === id);

test("the second item pins, or unpins when the process is pinned", () => {
  const unpinned = procMenuItems({ pid: 4, name: "a", command: "a" });
  assert.equal(unpinned[1].id, "pin");
  assert.equal(unpinned[1].labelKey, "proc.pin");
  assert.equal(unpinned[1].icon, "pin");
  const pinned = procMenuItems(
    { pid: 4, name: "a", command: "a" },
    { pinned: true },
  );
  assert.equal(pinned[1].labelKey, "proc.unpin");
  assert.equal(pinned[1].icon, "pinOff");
});

test("the six ids keep their order and end closes the list", () => {
  const items = procMenuItems({ pid: 4, name: "a", command: "a" });
  assert.deepEqual(ids(items), [
    "details",
    "pin",
    "copyName",
    "copyPid",
    "location",
    "end",
  ]);
  const last = items[items.length - 1];
  assert.deepEqual(last, item(items, "end"));
  assert.equal(last.danger, true);
  assert.equal(last.separator, true);
  assert.equal(
    items.slice(0, 5).some((entry) => entry.separator),
    false,
  );
});

test("a missing field disables only the action that needs it", () => {
  // The reveal sits after copyPid whatever the location state.
  assert.equal(procMenuItems(undefined).length, 6);
  assert.equal(ids(procMenuItems(undefined)).indexOf("location"), 4);
  assert.deepEqual(ids(procMenuItems(null)), [
    "details",
    "pin",
    "copyName",
    "copyPid",
    "location",
    "end",
  ]);
  const empty = procMenuItems({});
  assert.deepEqual(
    empty.filter((entry) => entry.disabled).map((entry) => entry.id),
    ["details", "pin", "copyName", "copyPid", "location", "end"],
  );
  const noName = procMenuItems({ pid: 4, command: "a" });
  assert.deepEqual(
    noName.filter((entry) => entry.disabled).map((entry) => entry.id),
    ["copyName", "location"],
  );
  const noCommand = procMenuItems({ pid: 4, name: "a" });
  assert.deepEqual(
    noCommand.filter((entry) => entry.disabled).map((entry) => entry.id),
    ["pin", "location"],
  );
  const noPid = procMenuItems({ name: "a", command: "a" });
  assert.deepEqual(
    noPid.filter((entry) => entry.disabled).map((entry) => entry.id),
    ["details", "copyPid", "location", "end"],
  );
  assert.equal(
    procMenuItems(
      { pid: 0, name: "a", command: "a" },
      {
        location: { kind: "ready" },
      },
    ).some((entry) => entry.disabled),
    false,
    "pid 0 is a real process",
  );
});

test("the reveal item sits right before the separator", () => {
  const items = procMenuItems(
    { pid: 4, name: "a", command: "a" },
    { location: { kind: "ready" } },
  );
  assert.equal(items.indexOf(item(items, "location")), 4);
  assert.equal(items[5].separator, true);
  assert.equal(item(items, "location").separator, undefined);
  assert.equal(item(items, "location").danger, undefined);
});

test("an unresolved location greys the reveal with its reason", () => {
  const process = { pid: 4, name: "a", command: "a" };
  const cases = [
    [undefined, "proc.locationChecking"],
    [{ kind: "checking" }, "proc.locationChecking"],
    [{ kind: "gone" }, "proc.gone"],
    [{ kind: "unreadable" }, "proc.locationUnreadable"],
    [{ kind: "failed" }, "proc.locationFailed"],
    [{ kind: "something else" }, "proc.locationChecking"],
  ];
  for (const [location, hintKey] of cases) {
    const entry = item(
      procMenuItems(process, location ? { location } : {}),
      "location",
    );
    assert.equal(entry.disabled, true, `${JSON.stringify(location)}`);
    assert.equal(entry.hintKey, hintKey);
    assert.equal(entry.labelKey, "proc.location");
    assert.equal(entry.icon, "folderOpen");
  }
  assert.equal(
    item(procMenuItems(process), "location").hintKey,
    "proc.locationChecking",
    "the default state is still checking",
  );
  assert.equal(
    procMenuItems(process, { location: { kind: "ready" } }).some(
      (entry) => entry.hintKey,
    ),
    false,
    "only the reveal item ever carries a hint",
  );
});

test("a ready location reveals that process through the shared context", () => {
  const calls = [];
  const seen = {
    ...ctx,
    onOpenLocation: (process) => calls.push(["location", process.pid]),
  };
  const process = { pid: 9, name: "a.exe", command: "a.exe" };
  const entry = item(
    procMenuItems(process, { location: { kind: "ready" } }),
    "location",
  );
  assert.equal(entry.disabled, undefined);
  assert.equal(entry.hintKey, undefined);
  entry.run(process, seen);
  assert.deepEqual(calls, [["location", 9]]);
});

test("an item runs that action on its process through the shared context", () => {
  const calls = [];
  const seen = {
    ...ctx,
    onShowDetails: (process) => calls.push(["details", process.pid]),
    onTogglePin: (command) => calls.push(["pin", command]),
    onKillProcess: (process) => calls.push(["end", process.pid]),
    copyText: (text) => calls.push(["copy", text]),
    onOpenLocation: (process) => calls.push(["location", process.pid]),
  };
  const process = { pid: 7, name: "game.exe", command: "game.exe" };
  for (const entry of procMenuItems(process, { location: { kind: "ready" } }))
    entry.run(process, seen);
  assert.deepEqual(calls, [
    ["details", 7],
    ["pin", "game.exe"],
    ["copy", "game.exe"],
    ["copy", "7"],
    ["location", 7],
    ["end", 7],
  ]);
});

test("the menu hangs down-right, flips at the far edges and stays inside", () => {
  const size = { width: 180, height: 200 };
  assert.deepEqual(
    menuPlacement({ x: 40, y: 30 }, size, { width: 900, height: 700 }),
    {
      left: 40,
      top: 30,
    },
  );
  assert.deepEqual(
    menuPlacement({ x: 880, y: 20 }, size, { width: 900, height: 700 }),
    { left: 700, top: 20 },
    "flips left of the pointer",
  );
  assert.deepEqual(
    menuPlacement({ x: 40, y: 690 }, size, { width: 900, height: 700 }),
    { left: 40, top: 490 },
    "flips above the pointer",
  );
  assert.deepEqual(
    menuPlacement({ x: 880, y: 690 }, size, { width: 900, height: 700 }),
    { left: 700, top: 490 },
    "both flips at the corner",
  );
  assert.deepEqual(
    menuPlacement({ x: 0, y: 0 }, size, { width: 900, height: 700 }),
    {
      left: 0,
      top: 0,
    },
  );
  const tiny = menuPlacement({ x: 100, y: 100 }, size, {
    width: 120,
    height: 90,
  });
  assert.equal(tiny.left, 0);
  assert.equal(tiny.top, 0);
  const negative = menuPlacement({ x: -50, y: -20 }, size, {
    width: 900,
    height: 700,
  });
  assert.ok(negative.left >= 0 && negative.top >= 0);
});

test("procRowOf finds the row that owns a target", () => {
  const row = {
    row: true,
    closest: (selector) => (selector === "[data-process-row]" ? row : null),
  };
  const child = {
    closest: (selector) => (selector === "[data-process-row]" ? row : null),
  };
  assert.equal(procRowOf(child), row);
  assert.equal(procRowOf(row), row, "the row itself");
  assert.equal(procRowOf(null), null);
  assert.equal(procRowOf(undefined), null);
  assert.equal(procRowOf({}), null, "a target without closest");
  assert.equal(
    procRowOf({ closest: () => null }),
    null,
    "a target outside the table",
  );
});
