import assert from "node:assert/strict";
import test from "node:test";
import {
  cursorAt,
  tooltipLeft,
  clockTime,
} from "../src/lib/utils/chart-cursor.js";

const at = (t, cpu = 1) => ({ timestamp: t, cpu });
test("the cursor snaps to the nearest sample on the chart", () => {
  const h = [at(0), at(60_000), at(117_000), at(120_000)];
  // Right edge = newest sample (120 s), left edge = 0.
  assert.equal(cursorAt(h, 1).sample, h[3]);
  assert.equal(cursorAt(h, 0).sample, h[0]);
  assert.equal(cursorAt(h, 0.49).sample, h[1]);
  assert.equal(cursorAt(h, 0.98).sample, h[2]); // 117.6 s → 117 s
  assert.equal(cursorAt(h, 0.5).x, 0.5);
  // Out-of-range ratios clamp; samples left of the window are never picked.
  const old = [at(-50_000), at(100_000), at(120_000)];
  assert.equal(cursorAt(old, -3).sample, old[1]);
  assert.equal(cursorAt(old, 9).sample, old[2]);
  // Gaps are samples too (the tooltip says "no reading").
  const gap = [at(0), at(60_000, null), at(120_000)];
  assert.equal(cursorAt(gap, 0.5).sample.cpu, null);
  // A short history sits at the right edge.
  assert.equal(cursorAt([at(5000)], 0).x, 1);
  assert.equal(cursorAt([], 0.5), null);
  assert.equal(cursorAt(h, 0.5, { end: 240_000 }).sample, h[3]);
});
test("the tooltip flips at the right edge and is never clipped", () => {
  assert.equal(tooltipLeft(10, 80, 300), 16); // right of the line
  assert.equal(tooltipLeft(250, 80, 300), 164); // flipped to the left
  assert.equal(tooltipLeft(50, 80, 120), 0); // neither side fits: clamped
  assert.equal(tooltipLeft(300, 80, 300), 214);
  assert.equal(tooltipLeft(5, 200, 150), 0); // wider than the box
  for (let x = 0; x <= 160; x += 1) {
    const l = tooltipLeft(x, 70, 160);
    assert.ok(l >= 0 && l + 70 <= 160, `x=${x}`);
  }
});
test("sample times read HH:mm:ss local", () => {
  const d = new Date(2026, 10, 3, 9, 5, 7).getTime();
  assert.equal(clockTime(d), "09:05:07");
});
