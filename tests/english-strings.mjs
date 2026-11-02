// Fob: no hard-coded user-visible English in Svelte components. Every
// string a user can read goes through $t(...) (src/lib/i18n/strings.js).
// Heuristic scan of each component (style blocks removed):
//  - text between tags, outside {expressions};
//  - string literals anywhere (script and markup) that read like words:
//    two or more words ("used of"), or one capitalised word ("Unavailable").
// Class lists, keys ("a.b"), identifiers, units and the allow-list pass;
// comparisons against data values (=== "Running") are not shown text.
// A line ending in `// i18n-ignore` is skipped (literal text such as an
// example search query that is the same in every language).
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const ROOTS = ["src/routes", "src/lib/components"];
const SKIP_DIRS = new Set(["ui"]); // shadcn primitives (no user text)
// Words that are the same in every locale, or not shown to users.
const ALLOW = new Set(
  `CPU GPU RAM RPM NVIDIA NVML PDH Fob NeoHtop PID PPID MB GB KB TB MHz Hz
  `.split(/\s+/),
);
const ALLOW_LITERALS = new Set([
  "Ctrl+,", // keyboard shortcut, printed the same in every locale
]);

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory())
      return SKIP_DIRS.has(name) ? [] : files(path);
    return name.endsWith(".svelte") ? [path] : [];
  });
}
const isClassList = (s) => {
  const tokens = s.trim().split(/\s+/);
  return (
    tokens.every((w) => /^[a-z0-9:[\]\-_/.!%()&>*#=,@'"~+]+$/.test(w)) &&
    tokens.some((w) => /[-:[]/.test(w))
  );
};
function wordy(s) {
  const text = s.replace(/\$\{[^}]*\}/g, " ").trim();
  if (!text || ALLOW_LITERALS.has(text)) return false;
  if (/^[\w.-]+$/.test(text) && /\./.test(text)) return false; // i18n key / path
  if (/^(\.|\/|\$|@|#|https?:|[a-z]+:\/\/)/.test(text)) return false;
  if (/^[\w-]+\.(svelte|js|ts|json|css|svg|png)$/.test(text)) return false;
  if (isClassList(text)) return false;
  const words = text.match(/[A-Za-z][A-Za-z']+/g) ?? [];
  const shown = words.filter((w) => !ALLOW.has(w));
  if (!shown.length) return false;
  if (/^[a-z][a-zA-Z0-9]*$/.test(text)) return false; // identifier / value
  if (/^[A-Z][a-z]+[A-Z]\w*$/.test(text)) return false; // CamelCase identifier
  const multi = /[A-Za-z]{2,}[\s,]+[A-Za-z]{2,}/.test(text);
  const capital = shown.some((w) => /^[A-Z][a-z]{2,}$/.test(w));
  return multi || capital;
}
/** Strip balanced {…} expressions from markup, keeping the text around them. */
function stripExpressions(markup) {
  let out = "";
  let depth = 0;
  for (const ch of markup) {
    if (ch === "{") depth++;
    else if (ch === "}") depth = Math.max(0, depth - 1);
    else if (!depth) out += ch;
  }
  return out;
}
const problems = [];
for (const file of ROOTS.flatMap(files)) {
  const src = readFileSync(file, "utf8")
    .replace(/<style[\s\S]*?<\/style>/g, (s) => s.replace(/[^\n]/g, ""))
    .split("\n")
    .map((line) => (/\/\/\s*i18n-ignore\b/.test(line) ? "" : line))
    .join("\n");
  const markup = src.replace(/<script[\s\S]*?<\/script>/g, "");
  const lines = (i) => src.slice(0, src.indexOf(i)).split("\n").length;
  // Text nodes.
  for (const m of stripExpressions(markup)
    .replace(/<!--[\s\S]*?-->/g, "")
    .matchAll(/>([^<>]+)</g)) {
    const text = m[1].replace(/&[a-z]+;/g, " ").trim();
    if (!text) continue;
    const words = (text.match(/[A-Za-z][A-Za-z']+/g) ?? []).filter(
      (w) => !ALLOW.has(w),
    );
    if (words.length) problems.push(`${file}: text "${text}"`);
  }
  // Literals (script, attribute values, expressions).
  const code = src
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/^\s*\/\/.*$/gm, "")
    .replace(/\/\*[\s\S]*?\*\//g, "");
  for (const m of code.matchAll(/(["'`])((?:\\.|(?!\1)[^\\\n])*)\1/g)) {
    const before = code.slice(Math.max(0, m.index - 40), m.index);
    if (/\bimport\b[^;]*$|from\s*$/.test(before)) continue;
    if (/\$?\bt\(\s*$/.test(before)) continue; // the key of $t("…")
    if (
      /\b(class|style|id|for|name|type|variant|size|side|align|role|kind|metric|rel|target|data-[\w-]+|tabindex|autocomplete|inputmode|lang)=\s*$/.test(
        before,
      )
    )
      continue;
    if (/[!=]==?\s*$/.test(before)) continue; // compared with a data value
    if (wordy(m[2]))
      problems.push(`${file}:${lines(m[0])}: ${m[0].slice(0, 80)}`);
    // Template literals: string literals inside ${…} are text too.
    if (m[1] === "`")
      for (const e of m[2].matchAll(/\$\{([^}]*)\}/g))
        for (const inner of e[1].matchAll(/(["'])((?:\\.|(?!\1)[^\\\n])*)\1/g))
          if (wordy(inner[2]))
            problems.push(`${file}:${lines(m[0])}: ${inner[0].slice(0, 80)}`);
  }
}
if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`${problems.length} hard-coded English strings (use $t)`);
}
assert.equal(problems.length, 0, "hard-coded English in components");
console.log("PASS: no hard-coded English in Svelte components");
