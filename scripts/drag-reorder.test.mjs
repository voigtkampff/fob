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

/** A point in a box's left corner. */
const leftSide = (box) => ({ x: box.left + 10, y: box.top + 10 });

const query = (boxes, from, at, options = {}) =>
  dropSlot({ items: boxes, from, x: at.x, y: at.y, ...options });

test("a card lands after its right neighbour from the right half alone, with the pointer high above its middle", () => {
  const grid = cards(4);
  // Card 1's top corner: the pointer is as far above card 1's middle as it
  // gets, and the old Y rule refused every drop there.
  const topRight = { x: grid[1].right - 10, y: grid[1].top + 1 };
  assert.deepEqual(
    query(grid, 0, topRight, { vertical: true, ignoreY: true }),
    {
      index: 1,
      after: true,
    },
  );
  const topLeft = { x: grid[1].left + 1, y: grid[1].top + 1 };
  assert.deepEqual(query(grid, 3, topLeft, { vertical: true, ignoreY: true }), {
    index: 1,
    after: false,
  });
  // The old rule asked for Y past the middle here, so this pointer gave nothing.
  assert.equal(query(grid, 0, topRight, { vertical: true }), null);
});

test("the same left half asks for the slot before its card", () => {
  const grid = cards(4);
  const topLeft = { x: grid[1].left + 1, y: grid[1].top + 1 };
  assert.deepEqual(query(grid, 2, topLeft, { vertical: true, ignoreY: true }), {
    index: 1,
    after: false,
  });
  // Forward over the same left half is the slot this card already holds.
  assert.equal(
    query(grid, 0, topLeft, { vertical: true, ignoreY: true }),
    null,
  );
});

test("a column without ignoreY still reads the side from Y", () => {
  const list = rows(4);
  const above = { x: 300, y: list[1].top + 5 };
  const below = { x: 300, y: list[1].bottom - 5 };
  assert.equal(query(list, 0, above, { vertical: true }), null);
  assert.deepEqual(query(list, 0, below, { vertical: true }), {
    index: 1,
    after: true,
  });
  assert.deepEqual(query(list, 3, above, { vertical: true }), {
    index: 1,
    after: false,
  });
});

test("the row under the pointer wins over a card whose column it matches", () => {
  const grid = cards(6);
  // Same column as card 1, but down in row 2 next to card 3.
  const secondRow = { x: grid[1].right - 10, y: grid[3].top + 40 };
  assert.deepEqual(
    query(grid, 0, secondRow, { vertical: true, ignoreY: true }),
    {
      index: 3,
      after: true,
    },
  );
  const secondRowLeft = { x: grid[3].left + 10, y: grid[3].top + 40 };
  assert.deepEqual(
    query(grid, 5, secondRowLeft, { vertical: true, ignoreY: true }),
    { index: 3, after: false },
  );
});

test("the first and last cards take a drop at their own edge", () => {
  const grid = cards(6);
  const first = { x: grid[0].left + 1, y: grid[0].top + 1 };
  const last = { x: grid[5].right - 1, y: grid[5].top + 1 };
  assert.deepEqual(query(grid, 5, first, { vertical: true, ignoreY: true }), {
    index: 0,
    after: false,
  });
  assert.deepEqual(query(grid, 0, last, { vertical: true, ignoreY: true }), {
    index: 5,
    after: true,
  });
});

test("a single row keeps using X without ignoreY", () => {
  const single = cards(3, 3);
  const right = { x: single[2].right - 10, y: single[2].top + 10 };
  assert.deepEqual(query(single, 0, right), { index: 2, after: true });
  assert.deepEqual(query(single, 2, leftSide(single[0])), {
    index: 0,
    after: false,
  });
});
