import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import ts from "typescript";
import { en, zh } from "../src/lib/i18n/strings.js";
import defaults from "../src/lib/definitions/default-config.json" with { type: "json" };
import {
  actionsWidth,
  columnKinds,
  columnWidth,
  nameMinWidth,
  tableWidth,
} from "../src/lib/utils/table-layout.js";

async function load(path, replace = {}) {
  let source = await readFile(new URL(path, import.meta.url), "utf8");
  for (const [from, to] of Object.entries(replace))
    source = source.replaceAll(`"${from}"`, `"${to}"`);
  const javascript = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  return `data:text/javascript;base64,${Buffer.from(javascript).toString("base64")}`;
}
const utilsUrl = await load("../src/lib/utils/index.ts");
const { sortProcesses } = await import(utilsUrl);
const { column_definitions: columns } = await import(
  await load("../src/lib/definitions/columns.ts", {
    "$lib/utils": utilsUrl,
    "$lib/utils/dashboard.js": new URL(
      "../src/lib/utils/dashboard.js",
      import.meta.url,
    ).href,
  })
);
const column = (id) => columns.find((c) => c.id === id);
const rows = [
  { pid: 1, command: "a", disk_read: null, disk_write: 0, gpu_usage: null },
  { pid: 2, command: "b", disk_read: 0, disk_write: null, gpu_usage: 0 },
  {
    pid: 3,
    command: "c",
    disk_read: 2_000_000,
    disk_write: 10,
    gpu_usage: 64.5,
  },
  { pid: 4, command: "d", disk_read: 500, disk_write: 9_000, gpu_usage: 3 },
];
const order = (field, direction) =>
  sortProcesses(rows, { field, direction }, new Set()).map((p) => p.pid);

test("busiest process sorts first and no reading sorts last both ways", () => {
  assert.deepEqual(order("disk_read", "desc"), [3, 4, 2, 1]);
  assert.deepEqual(order("disk_read", "asc"), [2, 4, 3, 1]);
  assert.deepEqual(order("disk_write", "desc"), [4, 3, 1, 2]);
  assert.deepEqual(order("gpu_usage", "desc"), [3, 4, 2, 1]);
  assert.deepEqual(order("gpu_usage", "asc"), [2, 4, 3, 1]);
});

test("speeds read as bytes per second and no reading is a dash, not 0", () => {
  for (const id of ["disk_read", "disk_write"]) {
    const format = column(id).format;
    assert.equal(format(null), "—");
    assert.equal(format(0), "0 B/s");
    assert.equal(format(1_048_576), "1.0 MB/s");
    assert.equal(format(512 * 1024), "512 KB/s");
    assert.ok(column(id).nullable && column(id).numeric);
  }
  assert.equal(column("gpu_usage").format(null), "—");
  assert.equal(column("gpu_usage").format(0), "0.0%");
  assert.equal(column("gpu_memory").format(null), "—");
  assert.equal(column("gpu_memory").format(1_342_177_280), "1280 MB");
});

test("GPU and both disk speeds show by default; the old I/O total column is gone", () => {
  for (const id of ["gpu_usage", "disk_read", "disk_write"]) {
    assert.equal(column(id).visible, true, id);
    assert.equal(defaults.appearance.columnVisibility[id], true, id);
  }
  assert.equal(column("gpu_memory").visible, false);
  assert.equal(column("disk_usage"), undefined);
  assert.equal(defaults.behavior.processColumns, 2);
  // preferences.rs keeps only saved columns that exist in the defaults.
  assert.deepEqual(
    Object.keys(defaults.appearance.columnVisibility).sort(),
    columns.map((c) => c.id).sort(),
  );
  const ids = columns.map((c) => c.id);
  assert.ok(ids.indexOf("gpu_usage") < ids.indexOf("virtual_memory"));
  assert.ok(ids.indexOf("disk_write") < ids.indexOf("virtual_memory"));
});

test("number and id columns take one width per kind of value", () => {
  for (const c of columns)
    if (c.numeric) assert.ok(columnKinds[c.id], `${c.id} has a width kind`);
  const byKind = {};
  for (const [id, kind] of Object.entries(columnKinds)) {
    assert.ok(column(id), `${id} is a column`);
    (byKind[kind] ??= new Set()).add(columnWidth(id));
  }
  for (const [kind, widths] of Object.entries(byKind))
    assert.equal(widths.size, 1, `${kind} columns share a width`);
  assert.equal(columnWidth("ppid"), columnWidth("pid"));
  assert.equal(columnWidth("gpu_usage"), columnWidth("cpu_usage"));
  assert.equal(columnWidth("gpu_memory"), columnWidth("memory_usage"));
  assert.equal(columnWidth("disk_write"), columnWidth("disk_read"));
});

const zhT = (key, vars = {}) =>
  zh[key].replace(/\{(\w+)\}/g, (_, name) => String(vars[name]));
// 12 px text: Consolas (Windows) is 0.55 em per character, Liberation Mono
// (the Linux preview) 0.6 em, plus 8 px cell padding each side.
const monoWidth = (text, em) => text.length * 12 * em + 16;

test("run time reads 0h 37m 45s in both languages and fits its column", () => {
  const runTime = column("run_time").format;
  assert.equal(runTime(37 * 60 + 45, zhT), "0h 37m 45s");
  assert.equal(runTime(7 * 3600 + 6 * 60 + 32, zhT), "7h 6m 32s");
  assert.equal(runTime(7 * 3600 + 6 * 60 + 32), "7h 6m 32s");
  assert.equal(zhT("dur.hms", { h: 7, m: 6, s: 32 }), "7时6分32秒");
  // A year of uptime, and 416 days as the widest value the preview checks.
  for (const seconds of [8760 * 3600 + 3599, 9999 * 3600 + 3599])
    for (const em of [0.55, 0.6])
      assert.ok(
        monoWidth(runTime(seconds, zhT), em) <= columnWidth("run_time") - 5,
        `${runTime(seconds, zhT)} at ${em} em`,
      );
});

test("the name column gives way before the table scrolls under the actions column", () => {
  const visible = Object.entries(defaults.appearance.columnVisibility).map(
    ([id, visible]) => ({ id, visible }),
  );
  for (const tree of [false, true]) {
    const others = visible.filter((c) => c.visible && c.id !== "name");
    const widest = Math.max(...others.map((c) => columnWidth(c.id, tree)));
    assert.equal(columnWidth("name", tree), nameMinWidth(tree));
    // Every data column shares the widest data column's minimum.
    assert.equal(
      tableWidth(visible, tree),
      actionsWidth + others.length * widest + nameMinWidth(tree),
    );
    // The default columns fit a 1920 px screen at 100% scaling; at 125%
    // (1536 CSS px) the equal widths scroll sideways a little.
    assert.ok(tableWidth(visible, tree) <= 1920, `tree ${tree}`);
  }
  assert.equal(tableWidth(visible), 1662);
  assert.equal(tableWidth(visible, true), 1742);
});

test("column labels say 硬盘 and Disk in both languages", () => {
  for (const c of columns) {
    assert.ok(en[`col.${c.id}`], `en col.${c.id}`);
    assert.ok(zh[`col.${c.id}`], `zh col.${c.id}`);
  }
  assert.equal(zh["col.disk_read"], "硬盘读取");
  assert.equal(zh["col.disk_write"], "硬盘写入");
  assert.equal(en["col.disk_read"], "Disk read");
  assert.equal(en["col.disk_write"], "Disk write");
  for (const [key, text] of Object.entries(zh))
    assert.ok(!text.includes("磁盘"), `zh ${key}: say 硬盘, not 磁盘`);
});

test("PawnIO is uninstalled in Windows settings, not in Fob", () => {
  assert.match(
    zh["setup.disableNote"],
    /PawnIO 驱动会保留.*Windows 系统设置中卸载/,
  );
  assert.match(
    en["setup.disableNote"],
    /PawnIO driver stays.*Windows Settings/,
  );
  assert.ok(!zh["setup.disableNote"].includes("设置 › 应用"));
  assert.ok(!en["setup.disableNote"].includes("Settings › Apps"));
});
