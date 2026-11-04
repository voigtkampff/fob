// winperf: the English and Chinese tables have the same keys and
// placeholders, and every key the UI uses exists.
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { en, zh, resolveLocale, translate } from "../src/lib/i18n/strings.js";
import { formatDuration, setupSteps } from "../src/lib/utils/dashboard.js";

const placeholders = (text) =>
  [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

assert.deepEqual(Object.keys(zh), Object.keys(en), "same keys, same order");
for (const [key, text] of Object.entries(zh))
  assert.ok(!text.includes("小窗"), `zh ${key}: say 迷你窗口, not 小窗`);
assert.ok(
  !readFileSync("src-tauri/src/i18n.rs", "utf8").includes("小窗"),
  "tray/native strings: say 迷你窗口, not 小窗",
);
for (const key of Object.keys(en)) {
  assert.ok(en[key].trim(), `en.${key} is empty`);
  assert.ok(zh[key].trim(), `zh.${key} is empty`);
  assert.deepEqual(
    placeholders(zh[key]),
    placeholders(en[key]),
    `placeholders of ${key}`,
  );
}

// Every literal key used in src/ exists ($t("…"), t("…"), $t(`prefix.${…}`)).
function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "i18n" ? [] : files(path);
    return /\.(svelte|ts|js)$/.test(name) ? [path] : [];
  });
}
const used = new Set();
const mentioned = new Set(); // any "a.b" literal, e.g. in t(cond ? "a.b" : "a.c")
const prefixes = new Set();
for (const file of files("src")) {
  const text = readFileSync(file, "utf8");
  for (const m of text.matchAll(/\$?\bt\(\s*"([\w.]+)"/g)) used.add(m[1]);
  for (const m of text.matchAll(/\$?\bt\(\s*`([\w.]+)\.\$\{/g))
    prefixes.add(m[1]);
  for (const m of text.matchAll(/"([a-z]+\.\w+)"/g)) mentioned.add(m[1]);
}
assert.ok(used.size > 140, `found only ${used.size} keys in use`);
for (const key of used) assert.ok(key in en, `missing key: ${key}`);
// Dynamic keys: the values they are built from.
const dynamic = {
  col: [
    ...readFileSync("src/lib/definitions/columns.ts", "utf8").matchAll(
      /id: "(\w+)"/g,
    ),
  ].map((m) => m[1]),
  detail: ["cpu", "gpu", "memory", "disk", "network", "temp"],
  lang: ["auto", "en", "zh"],
  "uptime.kind": ["cold", "fast", "hibernate"],
};
assert.deepEqual(
  [...prefixes].sort(),
  Object.keys(dynamic).sort(),
  "dynamic key prefixes",
);
for (const [prefix, ids] of Object.entries(dynamic))
  for (const id of ids)
    assert.ok(`${prefix}.${id}` in en, `missing key: ${prefix}.${id}`);
// Unused keys are dead weight.
const all = new Set([
  ...used,
  ...[...mentioned].filter((key) => key in en),
  ...Object.entries(dynamic).flatMap(([p, ids]) =>
    ids.map((id) => `${p}.${id}`),
  ),
]);
const unused = Object.keys(en).filter((key) => !all.has(key));
assert.deepEqual(unused, [], "unused keys");

// Locale choice and lookups.
assert.equal(resolveLocale("auto", "zh-CN"), "zh");
assert.equal(resolveLocale("auto", "zh-Hant-TW"), "zh");
assert.equal(resolveLocale("auto", "en-GB"), "en");
assert.equal(resolveLocale(undefined, "de-DE"), "en");
assert.equal(resolveLocale("en", "zh-CN"), "en");
assert.equal(resolveLocale("zh", "en-US"), "zh");
assert.equal(translate("zh", "dash.threads", { n: 24 }), "24 线程");
assert.equal(translate("zh", "no.such.key"), "no.such.key");
const tZh = (key, vars) => translate("zh", key, vars);
assert.equal(formatDuration(2 * 86400 + 21 * 3600, tZh), "2 天 21 小时");
assert.equal(formatDuration(3 * 3600 + 4 * 60), "3 hr, 4 min");
assert.match(
  setupSteps(
    { pawnio_installed: false, installed: false, task: "missing" },
    tZh,
  )[0],
  /PawnIO 驱动/,
);
console.log(
  `PASS: i18n tables (${Object.keys(en).length} keys, ${used.size} used literally) and locale choice`,
);
