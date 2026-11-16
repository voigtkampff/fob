import { test } from "node:test";
import assert from "node:assert/strict";
import { dropSlot } from "../src/lib/utils/drag-reorder.js";

const CARD_W = 300;
const CARD_H = 250;
const GAP = 10;

/** Cards as the dashboard lays them out: two per row, 250px tall. */
const cards = (count, columns = 2) =>
  Array.from({ length: count }, (_, i) => {
    const left = (i % columns) * (CARD_W + GAP);
    const top = Math.floor(i / columns) * (CARD_H + GAP);
    return { left, right: left + CARD_W, top, bottom: top + CARD_H };
  });

/** A settings list: one item per row, 60px tall. */
const rows = (count) =>
  Array.from({ length: count }, (_, i) => {
    const top = i * (60 + GAP);
    return { left: 0, right: 600, top, bottom: top + 60 };
  });

/** A table header: one row of columns, 40px tall, 100px wide. */
const columns = (count) =>
  Array.from({ length: count }, (_, i) => ({
    left: i * (100 + GAP),
    right: i * (100 + GAP) + 100,
    top: 0,
    bottom: 40,
  }));

/** Pointer heights inside a box, from its top edge to its bottom edge. */
const heights = (box) => [
  box.top + 1,
  (box.top + box.bottom) / 2 - 1,
  box.bottom - 1,
];

const query = (boxes, from, at, options = {}) =>
  dropSlot({ items: boxes, from, x: at.x, y: at.y, ...options });

test("a card takes a drop on either side at head height, whatever the pointer's Y", () => {
  const grid = cards(4);
  const target = grid[1];
  for (const y of heights(target)) {
    // Right half of the right neighbour, back and forward. The head of a card
    // is far above its middle, and that used to refuse every forward drop.
    assert.deepEqual(
      query(grid, 0, { x: target.right - 10, y }, { vertical: true }),
      { index: 1, after: true },
      `forward at y=${y}`,
    );
    assert.deepEqual(
      query(grid, 3, { x: target.left + 1, y }, { vertical: true }),
      { index: 1, after: false },
      `backward at y=${y}`,
    );
    // The half the card already holds asks for nothing, at any height.
    assert.equal(
      query(grid, 0, { x: target.left + 1, y }, { vertical: true }),
      null,
      `forward over the left half at y=${y}`,
    );
    assert.equal(
      query(grid, 3, { x: target.right - 10, y }, { vertical: true }),
      null,
      `backward over the right half at y=${y}`,
    );
  }
  // The failing case, spelled out: the pointer sits above the card's middle.
  const middle = (target.top + target.bottom) / 2;
  assert.ok(target.top + 1 < middle);
  assert.deepEqual(
    query(
      grid,
      0,
      { x: target.right - 10, y: target.top + 1 },
      { vertical: true },
    ),
    { index: 1, after: true },
  );
});

test("the summary cards read the card's own side, not the pointer's column across the grid", () => {
  const grid = cards(6);
  // Left half of the card one column further left still means its own slot.
  const third = grid[2];
  assert.deepEqual(
    query(grid, 5, { x: third.left + 1, y: third.top + 1 }, { vertical: true }),
    { index: 2, after: false },
  );
  assert.equal(
    query(grid, 1, { x: third.left + 1, y: third.top + 1 }, { vertical: true }),
    null,
  );
});

test("cards that wrap still pick the row under the pointer", () => {
  const grid = cards(6);
  const second = grid[3];
  assert.deepEqual(
    query(
      grid,
      0,
      { x: second.right - 10, y: second.top + 1 },
      { vertical: true },
    ),
    { index: 3, after: true },
  );
  assert.deepEqual(
    query(
      grid,
      5,
      { x: second.left + 1, y: second.top + 1 },
      { vertical: true },
    ),
    { index: 3, after: false },
  );
});

test("the settings list reads the side from Y at any X", () => {
  const list = rows(4);
  const target = list[1];
  for (const x of [10, 300, 590]) {
    assert.equal(
      query(
        list,
        0,
        { x, y: target.top + 5 },
        { vertical: true, stacked: true },
      ),
      null,
      `forward over the row's top half at x=${x}`,
    );
    assert.deepEqual(
      query(
        list,
        0,
        { x, y: target.bottom - 5 },
        { vertical: true, stacked: true },
      ),
      { index: 1, after: true },
      `forward over the row's bottom half at x=${x}`,
    );
    assert.deepEqual(
      query(
        list,
        3,
        { x, y: target.top + 5 },
        { vertical: true, stacked: true },
      ),
      { index: 1, after: false },
      `backward over the row's top half at x=${x}`,
    );
    assert.equal(
      query(
        list,
        3,
        { x, y: target.bottom - 5 },
        { vertical: true, stacked: true },
      ),
      null,
      `backward over the row's bottom half at x=${x}`,
    );
  }
});

test("a table header reads the side from X, at any height", () => {
  const header = columns(4);
  // No vertical: the header is one row, and the pointer's height never matters.
  for (const y of [-200, 1, 39, 400]) {
    assert.deepEqual(
      query(header, 0, { x: header[1].right - 10, y }),
      { index: 1, after: true },
      `forward at y=${y}`,
    );
    assert.deepEqual(
      query(header, 3, { x: header[1].left + 1, y }),
      { index: 1, after: false },
      `backward at y=${y}`,
    );
  }
});
