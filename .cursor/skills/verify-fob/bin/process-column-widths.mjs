// process-column-widths.mjs <evidence-dir> [tag]: renders the mocked process
// table (real ProcessTable, column widths, i18n and production CSS) with every
// column from the user's tree view and the longest value each column can show,
// in zh and en, flat and tree, default and wide column sets, at 1024 and
// 1600 px, at 1400 and 1480 px (just under the old default flat and tree
// table widths), and at 1920 px shown at 100%, 125% and 150% scaling (1920,
// 1536, 1280 CSS px). It measures each column, checks that no value runs under the
// sticky actions column (at rest when the table fits, scrolled to the end
// when it does not) and that the table only scrolls sideways when the window
// is narrower than its minimum width, and writes
// columns-<tag>-<lang>-<view>-<set>-<width>[-right].png plus
// columns-<tag>.json. The name column never grows past its widest (spare
// width goes after the actions column). Mocked readings only; this is not Windows evidence.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { en, zh } from "../../../../src/lib/i18n/strings.js";
import { nameMaxWidth } from "../../../../src/lib/utils/table-layout.js";

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
      for (const set of ["default", "wide"])
        for (const width of [1024, 1280, 1400, 1480, 1536, 1600, 1920]) {
          await send("Emulation.setDeviceMetricsOverride", {
            width,
            height: 420,
            deviceScaleFactor: 1,
            mobile: false,
          });
          await send("Page.navigate", {
            url: `${base}/process-table-panel.html?lang=${lang}&view=${view}${set === "wide" ? "&wide" : ""}`,
          });
          for (
            let i = 0;
            i < 100 &&
            (await evaluate("document.querySelectorAll('tbody tr').length")) <
              8;
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
            `(() => { const r = document.querySelector('.process-table-region'); return { scrollWidth: r.scrollWidth, clientWidth: r.clientWidth, minWidth: parseFloat(r.querySelector('table').style.width.match(/,\\s*([\\d.]+)px/)[1]) }; })()`,
          );
          // Gap between the last data column's text and the sticky actions
          // column in every row and the header; negative means it runs under.
          const actionsGap = () =>
            evaluate(
              `(() => { const rows = [...document.querySelectorAll('tbody tr, thead tr')]; return Math.round(Math.min(...rows.map((tr) => { const last = tr.children[tr.children.length - 2]; const ink = [...last.querySelectorAll('svg')].map((e) => e.getBoundingClientRect().right); const w = document.createTreeWalker(last, NodeFilter.SHOW_TEXT); for (let n; (n = w.nextNode());) if (n.textContent.trim()) { const g = document.createRange(); g.selectNodeContents(n); ink.push(g.getBoundingClientRect().right); } return tr.lastElementChild.getBoundingClientRect().left - Math.max(...ink); }))); })()`,
            );
          const fits = region.scrollWidth <= region.clientWidth;
          const restGap = await actionsGap();
          const name = `columns-${tag}-${lang}-${view}-${set}-${width}`;
          await shot(`${name}.png`);
          let endGap = restGap;
          if (!fits) {
            await evaluate(
              "(() => { const r = document.querySelector('.process-table-region'); r.scrollLeft = r.scrollWidth; })()",
            );
            await new Promise((r) => setTimeout(r, 200));
            endGap = await actionsGap();
            await shot(`${name}-right.png`);
          }
          report.push({
            lang,
            view,
            set,
            width,
            region,
            restGap,
            endGap,
            columns,
          });
          const at = `${lang}/${view}/${set}/${width}`;
          if (endGap < 8)
            problems.push(
              `${at}: last column text ${endGap}px from the actions column`,
            );
          if (!fits && region.clientWidth >= region.minWidth)
            problems.push(
              `${at}: scrolls sideways though ${region.clientWidth}px fits the ${region.minWidth}px minimum`,
            );
          const nameColumn = columns.find((c) => c.id === "name");
          if (nameColumn.width > nameMaxWidth(view === "tree") + 1)
            problems.push(
              `${at}: name column ${nameColumn.width}px, over its ${nameMaxWidth(view === "tree")}px widest`,
            );
          for (const c of columns) {
            const where = `${at} ${c.id}`;
            if (c.headClipped) problems.push(`${where}: header clipped`);
            else if (c.headNeed > c.width - 5)
              problems.push(`${where}: header has under 5 px to spare`);
            if (
              !["name", "user", "command", "actions"].includes(c.id) &&
              c.valueNeed > c.width - 5
            )
              problems.push(
                `${where}: longest value ${c.longest} needs ${c.valueNeed}px of ${c.width}`,
              );
            if (
              !["name", "user", "command"].includes(c.id) &&
              c.truncated.length
            )
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
  (r) =>
    r.lang === "en" &&
    r.view === "flat" &&
    r.set === "wide" &&
    r.width === 1024,
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
    `${r.lang}/${r.view}/${r.set}/${r.width}: scroll ${r.region.scrollWidth}/${r.region.clientWidth} (min ${r.region.minWidth}), actions gap ${r.restGap} at rest, ${r.endGap} at the end, need ${r.columns.map((c) => `${c.id}=${c.width}/${Math.max(c.headNeed ?? 0, c.valueNeed)}`).join(" ")}`,
  );
console.log(
  problems.length
    ? `FAIL\n${problems.join("\n")}`
    : "PASS: no clipped header, no truncated value, header and cells share alignment, nothing runs under the actions column",
);
process.exitCode = problems.length ? 1 : 0;
