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
