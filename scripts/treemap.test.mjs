// Fob: disk space map layout.
import test from "node:test";
import assert from "node:assert/strict";
import {
  squarify,
  blocks,
  folderAt,
  joinPath,
  fileKind,
} from "../src/lib/utils/treemap.js";

const rect = { x: 10, y: 20, w: 400, h: 250 };
const near = (a, b, eps = 1e-6) => Math.abs(a - b) <= eps * Math.max(1, b);

function check(values) {
  const out = squarify(values, (v) => v, rect);
  const total = values.filter((v) => v > 0).reduce((a, b) => a + b, 0);
  assert.equal(out.length, values.filter((v) => v > 0).length);
  let area = 0;
  for (const r of out) {
    area += r.w * r.h;
    // Inside the bounds.
    assert.ok(r.x >= rect.x - 1e-6 && r.y >= rect.y - 1e-6, "inside");
    assert.ok(r.x + r.w <= rect.x + rect.w + 1e-6, "inside right");
    assert.ok(r.y + r.h <= rect.y + rect.h + 1e-6, "inside bottom");
    // Area proportional to the value.
    assert.ok(near((r.w * r.h) / (rect.w * rect.h), r.item / total, 1e-6));
  }
  assert.ok(near(area, rect.w * rect.h), "fills the rectangle");
  // No two rectangles overlap.
  for (let i = 0; i < out.length; i++)
    for (let j = i + 1; j < out.length; j++) {
      const a = out[i];
      const b = out[j];
      const ox = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
      const oy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
      assert.ok(ox <= 1e-6 || oy <= 1e-6, `rects ${i} and ${j} overlap`);
    }
  return out;
}

test("squarify: bounds, proportional areas, no overlap", () => {
  check([6, 6, 4, 3, 2, 2, 1]);
  check([1000, 1, 1, 1]);
  check(Array.from({ length: 300 }, (_, i) => ((i * 7919) % 1000) + 1));
  check([5, 0, -1, NaN, 3].map(Number));
  assert.deepEqual(
    squarify([], (v) => v, rect),
    [],
  );
  assert.deepEqual(
    squarify([1], (v) => v, { x: 0, y: 0, w: 0, h: 5 }),
    [],
  );
});

test("squarify keeps blocks close to square", () => {
  const ratio = (r) => Math.max(r.w / r.h, r.h / r.w);
  // 16 equal items in a square: a 4x4 grid of squares.
  const grid = squarify(Array(16).fill(1), (v) => v, {
    x: 0,
    y: 0,
    w: 100,
    h: 100,
  });
  assert.ok(grid.every((r) => ratio(r) < 1.01));
  // The paper's example: worst ratio well below a slice layout's.
  const out = check([6, 6, 4, 3, 2, 2, 1]);
  assert.ok(Math.max(...out.map(ratio)) < 3.5);
  assert.equal(out[0].item, 6, "largest first");
});

const folder = {
  n: "C:\\",
  s: 1000,
  f: 10,
  d: true,
  c: [
    {
      n: "Windows",
      s: 600,
      f: 5,
      d: true,
      c: [{ n: "System32", s: 500, f: 4, d: true }],
    },
    { n: "pagefile.sys", s: 300, f: 1 },
    { n: "tiny.txt", s: 1, f: 1 },
    { n: "Users", s: 59, f: 2, d: true },
  ],
  o: 40,
  oc: 7,
};

test("blocks: large items kept, the rest folded into one block", () => {
  const out = blocks(folder, { area: 60_000 });
  assert.deepEqual(
    out.map((b) => b.node?.n ?? null),
    ["Windows", "pagefile.sys", "Users", null],
  );
  const other = out.at(-1);
  assert.equal(other.size, 41, "tiny.txt + what the scan folded");
  assert.equal(other.count, 8);
  assert.equal(
    out.reduce((sum, b) => sum + b.size, 0),
    folder.s,
    "nothing lost",
  );
  const capped = blocks(folder, { area: 1e12, maxItems: 1 });
  assert.equal(capped.length, 2);
  assert.equal(capped[1].size, 400);
  assert.deepEqual(
    blocks({ n: "empty", s: 0, f: 0, d: true }, { area: 1e4 }),
    [],
  );
});

test("paths, drill-down lookup and file types", () => {
  assert.equal(folderAt(folder, ["Windows", "System32"]).n, "System32");
  assert.equal(
    folderAt(folder, ["pagefile.sys"]),
    null,
    "files are not folders",
  );
  assert.equal(folderAt(folder, ["Gone"]), null);
  assert.equal(
    joinPath("C:\\", ["Windows", "System32"], "\\"),
    "C:\\Windows\\System32",
  );
  assert.equal(joinPath("/workspace", ["a", "b"], "/"), "/workspace/a/b");
  assert.equal(joinPath("/workspace", [], "/"), "/workspace");
  assert.equal(fileKind("movie.MKV"), "video");
  assert.equal(fileKind("pagefile.sys"), "program");
  assert.equal(fileKind(".gitignore"), "other");
  assert.equal(fileKind("README"), "other");
});
