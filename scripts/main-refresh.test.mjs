import assert from "node:assert/strict";
import test from "node:test";
import { mainRefresh } from "../src/lib/utils/main-refresh.js";

const counting = () => {
  const r = { reads: 0 };
  r.refresh = mainRefresh(() => r.reads++);
  return r;
};

test("ticks read only while the window is on screen", () => {
  const r = counting();
  r.refresh.tick(true);
  r.refresh.tick(false);
  r.refresh.tick(false);
  assert.equal(r.reads, 1);
});

test("the first focus after a hidden tick reads at once, once", () => {
  const r = counting();
  r.refresh.tick(false);
  r.refresh.shown();
  assert.equal(r.reads, 1);
  r.refresh.shown();
  assert.equal(r.reads, 1, "switching focus back and forth reads nothing more");
});

test("focus without a missed tick reads nothing", () => {
  const r = counting();
  r.refresh.tick(true);
  r.refresh.shown();
  assert.equal(r.reads, 1);
});

test("hiding and showing again and again never leaves reads paused", () => {
  const r = counting();
  for (let i = 0; i < 5; i++) {
    r.refresh.tick(false);
    r.refresh.shown();
  }
  assert.equal(r.reads, 5);
  r.refresh.tick(true);
  assert.equal(r.reads, 6, "ticks read again once the window stays on screen");
});
