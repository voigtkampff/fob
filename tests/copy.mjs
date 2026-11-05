// UI copy rule (0.7.7): no 。 anywhere, and no English sentence that ends
// with a period. Ellipses and numbers such as 1.5 are fine.
import assert from "node:assert/strict";
import { en, zh } from "../src/lib/i18n/strings.js";

/** @param {string} text */
export function copyProblem(text) {
  if (text.includes("。")) return "contains 。";
  const plain = text.replace(/\.{3}|…/g, "");
  if (/\.\s*$/.test(plain)) return "ends with a period";
  if (/\.\s+\S/.test(plain)) return "has a period between sentences";
  return null;
}

assert.equal(copyProblem("Waiting…"), null);
assert.equal(copyProblem("Loading..."), null);
assert.equal(copyProblem("v1.5 · 2.0 GB"), null);
assert.equal(copyProblem("Done."), "ends with a period");
assert.equal(copyProblem("One. Two"), "has a period between sentences");
assert.equal(copyProblem("完成。"), "contains 。");

const problems = [];
for (const [lang, table] of Object.entries({ en, zh }))
  for (const [key, text] of Object.entries(table)) {
    const why = copyProblem(text);
    if (why) problems.push(`${lang} ${key}: ${why}: ${text}`);
  }
assert.deepEqual(problems, [], problems.join("\n"));
console.log(`PASS: UI copy has no 。 or sentence-ending periods (${Object.keys(en).length * 2} strings)`);
