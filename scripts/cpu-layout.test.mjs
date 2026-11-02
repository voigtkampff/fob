import assert from "node:assert/strict";
import test from "node:test";
import {
  coreLayout,
  busiestThreads,
  CARD_AREA,
  CARD_ROW,
  CARD_MIN_CELL,
  CARD_GAP,
} from "../src/lib/utils/cpu-layout.js";

test("the CPU card picks a layout by thread count", () => {
  assert.deepEqual(coreLayout(8), {
    mode: "cells",
    cols: 8,
    rows: 1,
    labels: true,
  });
  assert.deepEqual(coreLayout(24), {
    mode: "cells",
    cols: 8,
    rows: 3,
    labels: true,
  });
  assert.deepEqual(coreLayout(32), {
    mode: "heatmap",
    cols: 16,
    rows: 2,
    labels: false,
  });
  assert.deepEqual(coreLayout(64), {
    mode: "heatmap",
    cols: 16,
    rows: 4,
    labels: false,
  });
  assert.deepEqual(coreLayout(128), { mode: "summary", top: 3 });
  assert.equal(coreLayout(4).cols, 4);
});
test("every card layout fits the fixed area without scrolling", () => {
  for (const n of [1, 2, 4, 6, 8, 12, 16, 20, 24, 28, 32, 48, 64]) {
    const l = coreLayout(n);
    assert.notEqual(l.mode, "summary", `${n}`);
    const rowH = CARD_ROW[l.mode];
    assert.ok(
      l.rows * rowH <= CARD_AREA.height,
      `${n} threads: ${l.rows} rows`,
    );
    const gap = CARD_GAP[l.mode];
    const cell = (CARD_AREA.width - gap * (l.cols - 1)) / l.cols;
    assert.ok(
      cell >= CARD_MIN_CELL[l.mode],
      `${n} threads: ${cell.toFixed(1)} px cells`,
    );
  }
});
test("detail views show every thread, 24 as labeled bars", () => {
  assert.deepEqual(coreLayout(24, "detail"), {
    mode: "cells",
    cols: 16,
    rows: 2,
    labels: true,
  });
  assert.equal(coreLayout(32, "detail").mode, "cells");
  assert.deepEqual(coreLayout(64, "detail"), {
    mode: "heatmap",
    cols: 32,
    rows: 2,
    labels: false,
  });
  assert.deepEqual(coreLayout(128, "detail"), {
    mode: "heatmap",
    cols: 32,
    rows: 4,
    labels: false,
  });
});
test("busiest threads, busiest first", () => {
  assert.deepEqual(busiestThreads([5, 90, NaN, 90, 1], 2), [
    { index: 1, value: 90 },
    { index: 3, value: 90 },
  ]);
});
