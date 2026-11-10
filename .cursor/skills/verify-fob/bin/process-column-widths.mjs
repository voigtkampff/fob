// process-column-widths.mjs <evidence-dir> [tag]: renders the mocked process
// table (real ProcessTable, column widths, i18n and production CSS) with every
// column from the user's tree view and the longest value each column can show,
// in zh and en, flat and tree, at 1024 and 1600 px. It measures each column
// and writes columns-<tag>-<lang>-<view>-<width>[-right].png plus
// columns-<tag>.json. Mocked readings only; this is not Windows evidence.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { en, zh } from "../../../../src/lib/i18n/strings.js";

const [out = "/workspace/scratch/verify/evidence", tag = "audit"] =
  process.argv.slice(2);
mkdirSync(out, { recursive: true });
const server = await createServer({
  configFile: fileURLToPath(
    new URL(
      "../../../../tests/fixtures/helper-settings.vite.config.js",
      import.meta.url,
    ),
  ),
  server: { host: "127.0.0.1", port: 0, strictPort: true },
  logLevel: "error",
});
await server.listen();
const base = `http://127.0.0.1:${server.httpServer.address().port}`;
const port = 9310;
const chrome = spawn(
  "google-chrome",
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    "--hide-scrollbars",
    `--remote-debugging-port=${port}`,
    "about:blank",
  ],
  { stdio: "ignore" },
);
const idOf = (strings, label) =>
  ({
    [strings["col.familyCpu"]]: "cpu_usage",
    [strings["col.familyRam"]]: "memory_usage",
  })[label] ??
  Object.keys(strings)
    .find((k) => k.startsWith("col.") && strings[k] === label)
    ?.slice(4) ??
  label;
const problems = [];
const report = [];
try {
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await new Promise((r) => setTimeout(r, 200));
    target = await fetch(`http://127.0.0.1:${port}/json`)
      .then((r) => r.json())
      .then((l) => l.find((t) => t.type === "page"))
      .catch(() => undefined);
  }
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener("open", r, { once: true }));
  let id = 0;
  const pending = new Map();
  ws.addEventListener("message", (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) pending.get(m.id)(m), pending.delete(m.id);
  });
  const send = (method, params = {}) =>
    new Promise((resolve) => {
      pending.set(++id, resolve);
      ws.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) =>
    (
      await send("Runtime.evaluate", {
        expression,
        returnByValue: true,
        awaitPromise: true,
      })
    ).result.result.value;
  const shot = async (name) =>
    writeFileSync(
      `${out}/${name}`,
      Buffer.from(
        (await send("Page.captureScreenshot", { format: "png" })).result.data,
        "base64",
      ),
    );
  for (const lang of ["zh", "en"])
    for (const view of ["flat", "tree"])
      for (const width of [1024, 1600]) {
        await send("Emulation.setDeviceMetricsOverride", {
          width,
          height: 420,
          deviceScaleFactor: 1,
          mobile: false,
        });
        await send("Page.navigate", {
          url: `${base}/process-table-panel.html?lang=${lang}&view=${view}&wide`,
        });
        for (
          let i = 0;
          i < 100 &&
          (await evaluate("document.querySelectorAll('tbody tr').length")) < 8;
          i++
        )
          await new Promise((r) => setTimeout(r, 100));
        await new Promise((r) => setTimeout(r, 300));
        const columns = await evaluate(`(() => {
          const natural = (el) => { const old = el.style.cssText; el.style.maxWidth = 'none'; el.style.width = 'max-content'; const w = el.getBoundingClientRect().width; el.style.cssText = old; return w; };
          const px = (el, prop) => parseFloat(getComputedStyle(el)[prop]) || 0;
          const textWidth = (el) => { const w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT); let l = Infinity, r = -Infinity; for (let n; (n = w.nextNode());) { if (!n.textContent.trim()) continue; const g = document.createRange(); g.selectNodeContents(n); const b = g.getBoundingClientRect(); l = Math.min(l, b.left); r = Math.max(r, b.right); } return r > l ? r - l : 0; };
          const right = (el) => { const r = document.createRange(); r.selectNodeContents(el); return Math.max(...[...r.getClientRects()].map((x) => x.right)); };
          const heads = [...document.querySelectorAll('thead th')];
          const rows = [...document.querySelectorAll('tbody tr')];
          return heads.map((th, i) => {
            const button = th.querySelector('button');
            const label = (button ?? th).textContent.trim();
            const box = th.getBoundingClientRect();
            const cells = rows.map((tr) => tr.children[i]);
            const inner = cells.map((td) => td.firstElementChild ?? td);
            return {
              label,
              width: Math.round(box.width),
              headAlign: getComputedStyle(th).textAlign.replace('start', 'left'),
              cellAlign: getComputedStyle(cells[0]).textAlign.replace('start', 'left'),
              headClipped: !!button && (natural(button) > button.getBoundingClientRect().width + 0.5 || button.getBoundingClientRect().right > box.right + 0.5 || button.getBoundingClientRect().left < box.left - 0.5),
              headSlack: button ? Math.round(box.width - button.scrollWidth - 16) : null,
              headRightGap: button ? Math.round(box.right - button.getBoundingClientRect().right) : null,
              cellRightGap: Math.round(Math.min(...cells.map((td) => box.right - right(td)))),
              truncated: inner.filter((d) => d.scrollWidth > d.clientWidth + 0.5).map((d) => d.textContent.trim()),
              headNeed: button ? Math.ceil(natural(button) + px(th, 'paddingLeft') + px(th, 'paddingRight') + px(button, 'marginLeft') + px(button, 'marginRight')) : null,
              headEdgeOffset: button ? Math.round(getComputedStyle(th).textAlign === 'right'
                ? (button.getBoundingClientRect().right - px(button, 'paddingRight')) - (cells[0].getBoundingClientRect().right - px(cells[0], 'paddingRight'))
                : (button.getBoundingClientRect().left + px(button, 'paddingLeft')) - (cells[0].getBoundingClientRect().left + px(cells[0], 'paddingLeft'))) : null,
              valueNeed: Math.ceil(Math.max(...cells.map(textWidth)) + 16),
              longest: inner.map((d) => d.textContent.trim()).sort((a, b) => b.length - a.length)[0],
            };
          });
        })()`);
        const strings = lang === "zh" ? zh : en;
        for (const c of columns) c.id = idOf(strings, c.label);
        const region = await evaluate(
          `(() => { const r = document.querySelector('.process-table-region'); return { scrollWidth: r.scrollWidth, clientWidth: r.clientWidth }; })()`,
        );
        report.push({ lang, view, width, region, columns });
        const name = `columns-${tag}-${lang}-${view}-${width}`;
        await shot(`${name}.png`);
        if (region.scrollWidth > region.clientWidth) {
          await evaluate(
            "(() => { const r = document.querySelector('.process-table-region'); r.scrollLeft = r.scrollWidth; })()",
          );
          await new Promise((r) => setTimeout(r, 200));
          await shot(`${name}-right.png`);
        }
        for (const c of columns) {
          const where = `${lang}/${view}/${width} ${c.id}`;
          if (c.headClipped) problems.push(`${where}: header clipped`);
          else if (c.headNeed > c.width - 5)
            problems.push(`${where}: header has under 5 px to spare`);
          if (!["name", "user", "command"].includes(c.id) && c.truncated.length)
            problems.push(`${where}: value truncated ${c.truncated[0]}`);
          if (c.headEdgeOffset)
            problems.push(
              `${where}: header text ${c.headEdgeOffset}px off the cell text edge`,
            );
          if (c.headAlign !== c.cellAlign && c.id !== "actions")
            problems.push(
              `${where}: header ${c.headAlign}, cells ${c.cellAlign}`,
            );
        }
      }
  writeFileSync(`${out}/columns-${tag}.json`, JSON.stringify(report, null, 2));
  ws.close();
} finally {
  chrome.kill();
  await server.close();
}
const need = {};
for (const r of report)
  for (const c of r.columns) {
    const n = (need[c.id] ??= {
      id: c.id,
      width: c.width,
      headNeed: 0,
      valueNeed: 0,
      headEdgeOffset: c.headEdgeOffset,
    });
    n.headNeed = Math.max(n.headNeed, c.headNeed ?? 0);
    n.valueNeed = Math.max(n.valueNeed, c.valueNeed);
  }
console.table(Object.values(need));
const flat = report.find(
  (r) => r.lang === "en" && r.view === "flat" && r.width === 1024,
);
console.table(
  flat.columns.map(({ id, width, headAlign, cellAlign, longest }) => ({
    id,
    width,
    headAlign,
    cellAlign,
    longest,
  })),
);
for (const r of report)
  console.log(
    `${r.lang}/${r.view}/${r.width}: scroll ${r.region.scrollWidth}/${r.region.clientWidth}, need ${r.columns.map((c) => `${c.id}=${c.width}/${Math.max(c.headNeed ?? 0, c.valueNeed)}`).join(" ")}`,
  );
console.log(
  problems.length
    ? `FAIL\n${problems.join("\n")}`
    : "PASS: no clipped header, no truncated value, header and cells share alignment",
);
process.exitCode = problems.length ? 1 : 0;
