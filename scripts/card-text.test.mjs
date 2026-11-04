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
    // Every opening tag whose class has `truncate` also has a title.
    for (const tag of src.match(/<[a-z]+\b[^>]*\btruncate\b[^>]*>/g) ?? []) {
      assert.match(tag, /\btitle=|\{title\}/, `${file}: ${tag}`);
    }
    assert.doesNotMatch(src, /break-words/, file);
  }
  const item = readFileSync(dir + "StatItem.svelte", "utf8");
  assert.equal((item.match(/\btruncate\b/g) ?? []).length, 2);
});
