import test from "node:test";
import assert from "node:assert/strict";
import {
  changedLeaves,
  mergePreferences,
} from "../src/lib/utils/settings-patch.js";
test("settings patches carry only edited leaves from a full settings group", () => {
  const base = {
    behavior: { refreshRate: 3000, processView: "flat" },
    appearance: { columnVisibility: { pid: true, name: true } },
  };
  const patch = changedLeaves(base, {
    behavior: { ...base.behavior, processView: "tree" },
  });
  assert.deepEqual(patch, { behavior: { processView: "tree" } });
  const otherWindow = mergePreferences(base, {
    behavior: { refreshRate: 5000 },
  });
  assert.deepEqual(mergePreferences(otherWindow, patch).behavior, {
    refreshRate: 5000,
    processView: "tree",
  });
  const columns = changedLeaves(base, {
    appearance: { columnVisibility: { pid: false, name: true } },
  });
  assert.deepEqual(columns, {
    appearance: { columnVisibility: { pid: false } },
  });
  assert.deepEqual(changedLeaves(base, base), {});
});

test("column widths go as one value, so a width reset to automatic is dropped", () => {
  const base = {
    behavior: { refreshRate: 2000, columnWidths: { pid: 90, status: 176 } },
  };
  const reset = changedLeaves(base, {
    behavior: { ...base.behavior, columnWidths: { pid: 90 } },
  });
  assert.deepEqual(reset, { behavior: { columnWidths: { pid: 90 } } });
  assert.deepEqual(mergePreferences(base, reset).behavior, {
    refreshRate: 2000,
    columnWidths: { pid: 90 },
  });
  const same = changedLeaves(base, {
    behavior: { ...base.behavior, columnWidths: { pid: 90, status: 176 } },
  });
  assert.deepEqual(same, {});
  const first = changedLeaves(
    { behavior: {} },
    { behavior: { columnWidths: { pid: 90 } } },
  );
  assert.deepEqual(first, { behavior: { columnWidths: { pid: 90 } } });
});
