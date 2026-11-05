import assert from "node:assert/strict";
import { blocks, MIN_TILE_AREA } from "../src/lib/utils/treemap.js";

const folder = {
  n: "f", s: 1000, f: 5, d: true, o: 10, oc: 7,
  c: [
    { n: "big", s: 700, f: 1 },
    { n: "mid", s: 200, f: 1, d: true },
    { n: "small", s: 60, f: 1 },
    { n: "tiny", s: 30, f: 1 },
  ],
};
// 100 x 100 px: tiny = 3 % = 300 px² < 600, small = 6 % = 600 px².
let b = blocks(folder, { area: 10_000 });
assert.deepEqual(b.filter((x) => x.node).map((x) => x.node.n), ["big", "mid", "small"]);
let rest = b.at(-1);
assert.equal(rest.node, null);
assert.equal(rest.size, 40, "folded bytes + merged items");
assert.equal(rest.count, 8, "scan-folded count + merged items");
assert.deepEqual(rest.merged.map((m) => m.n), ["tiny"]);
// A smaller map merges more: 50 x 50 px (mid = 500 px²).
b = blocks(folder, { area: 2_500 });
assert.deepEqual(b.at(-1).merged.map((m) => m.n), ["mid", "small", "tiny"]);
assert.equal(MIN_TILE_AREA, 600);
assert.equal(b.reduce((s, x) => s + x.size, 0), 1000, "blocks cover the folder");
console.log("PASS: treemap merges by pixel area; other block keeps its items");

{
  const { squarify } = await import("../src/lib/utils/treemap.js");
  const rect = { x: 0, y: 0, w: 400, h: 300 };
  const items = [
    { node: null, size: 600 },
    ...[200, 120, 80, 50, 30].map((size, i) => ({ node: { n: `k${i}` }, size })),
  ];
  for (const r of [rect, { x: 0, y: 0, w: 300, h: 400 }]) {
    const out = squarify(items, (b) => b.size, r, { last: (b) => !b.node });
    const other = out.at(-1);
    assert.equal(other.item.node, null, "other is laid out last");
    const close = (a, b) => Math.abs(a - b) < 1e-6;
    assert.ok(close(other.x + other.w, r.w) && close(other.y + other.h, r.h), "other touches the bottom-right corner");
    const total = items.reduce((s, b) => s + b.size, 0);
    for (const t of out)
      assert.ok(close(t.w * t.h, (t.item.size / total) * r.w * r.h), "areas stay proportional");
  }
  const plain = squarify(items, (b) => b.size, rect);
  assert.equal(plain[0].item.node, null, "without the option the largest still goes first");
  console.log("PASS: the other block goes last, to the bottom-right, areas kept");
}
