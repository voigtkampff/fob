// Fob: every stats-row card opens the same kind of side panel, laid out by
// one shared component (no per-panel layout code, no page navigation).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const dir = new URL("../src/lib/components/stats/", import.meta.url);
const read = (name) => readFileSync(new URL(name, dir), "utf8");
const kinds = {
  CpuPanel: "cpu",
  MemoryPanel: "memory",
  GpuPanel: "gpu",
  StoragePanel: "storage",
  SystemPanel: "system",
  NetworkPanel: "network",
  TempPanel: "temp",
};

test("each card's arrow opens its side panel", () => {
  for (const [file, kind] of Object.entries(kinds)) {
    const src = read(`${file}.svelte`);
    assert.match(src, new RegExp(`<ResourceDetails\\s+kind="${kind}"`), file);
    assert.doesNotMatch(src, /ArrowUpRight/, `${file} has its own arrow`);
    assert.doesNotMatch(
      src,
      /\{#snippet children\(\)\}/,
      `${file} lays out its own panel`,
    );
  }
});

test("every panel body goes through DetailLayout", () => {
  const src = read("ResourceDetailsContent.svelte");
  const branches = [
    ...src.matchAll(/\{(?:#if|:else if) kind === "(\w+)"/g),
  ].map((m) => m[1]);
  assert.deepEqual([...new Set(branches)].sort(), Object.values(kinds).sort());
  assert.ok((src.match(/<DetailLayout\b/g) ?? []).length >= branches.length);
  // Cards come from DetailLayout / DetailSection only.
  assert.doesNotMatch(src, /<Card\./);
  const kindsType = read("panel.ts");
  for (const kind of Object.values(kinds))
    assert.match(kindsType, new RegExp(`"${kind}"`));
});

test("the scrolling body pads its top so a first card keeps its ring", () => {
  const src = read("ResourceDetails.svelte");
  const body = src.match(/<div class="([^"]*overflow-y-auto[^"]*)"/);
  assert.ok(body, "scroll body");
  assert.match(body[1], /\bpt-\d/);
  assert.match(body[1], /\bpb-\d/);
});
