import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { shortOs } from "../src/lib/utils/os-name.js";

const dir = "src/lib/components/stats/";
const cards = readdirSync(dir).filter((f) =>
  /^(StatCard|StatItem|\w+Panel)\.svelte$/.test(f),
);

test("OS names get shorter for the System card, others are only trimmed", () => {
  assert.equal(
    shortOs("Windows 11 Pro for Workstations"),
    "Win 11 Pro for Workstations",
  );
  assert.equal(shortOs("Microsoft Windows 10 Home"), "Win 10 Home");
  assert.equal(
    shortOs("  Linux (Debian  GNU/Linux 13) "),
    "Linux (Debian GNU/Linux 13)",
  );
  assert.equal(shortOs("WindowsXYZ"), "WindowsXYZ");
});

test("card text never wraps: truncated lines carry their full text as a tooltip", () => {
  assert.ok(cards.length >= 9, cards.join());
  for (const file of cards) {
    const src = readFileSync(dir + file, "utf8");
    // Every opening tag whose class has `truncate` is a tooltip trigger: it
    // sits in the Tip that carries its full text, or keeps a title of its own.
    const ranges = [...src.matchAll(/<Tip\b/g)].map(({ index }) => {
      const close = src.indexOf("</Tip>", index);
      return [index, close === -1 ? src.length : close];
    });
    for (const tag of src.matchAll(/<[a-z]+\b[^>]*\btruncate\b[^>]*>/g)) {
      const inTip = ranges.some(
        ([open, close]) => tag.index > open && tag.index < close,
      );
      assert.ok(
        /\btitle=|\{title\}/.test(tag[0]) || inTip,
        `${file}: ${tag[0].replace(/\s+/g, " ")}`,
      );
    }
    assert.doesNotMatch(src, /break-words/, file);
  }
  const item = readFileSync(dir + "StatItem.svelte", "utf8");
  assert.equal((item.match(/\btruncate\b/g) ?? []).length, 2);
});
