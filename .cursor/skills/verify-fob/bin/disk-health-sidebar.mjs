// disk-health-sidebar.mjs <evidence-dir> [tag]: the real main window route
// (+page.svelte, production CSS) with generic mocked readings. Opens the
// Processes tab's storage panel with real pointer events and checks the
// Physical disks summary: one entry per physical disk (not per volume), only
// the fields each disk reports, every state explained, nothing clipped, in zh
// and en at device scale 1, 1.25 and 1.5. Then clicks 查看健康详情 / View
// health details and checks the panel closed, the page lock is released, the
// Dashboard tab shows the Disk detail with focus on its health section, and
// that the 5 minute refresh timer exists only while a health view is open.
// Mocked readings only; this is not Windows evidence.
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
const port = 9312;
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
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const waitFor = async (expr, ms = 8000) => {
    for (let t = 0; t < ms; t += 100) {
      if (await evaluate(expr)) return true;
      await sleep(100);
    }
    return false;
  };
  const click = async (selector) => {
    const box = await evaluate(
      `(() => { const e = [...document.querySelectorAll(${JSON.stringify(selector)})].find((e) => e.getClientRects().length); if (!e) return null; e.scrollIntoView({ block: "center" }); const r = e.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
    );
    if (!box) return false;
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
      await send("Input.dispatchMouseEvent", {
        type,
        x: box.x,
        y: box.y,
        button: "left",
        clickCount: 1,
      });
    return true;
  };
  const SHEET = '[data-slot="sheet-content"]';
  const summary = `[...document.querySelectorAll('${SHEET} [data-slot="card"]')].find((c) => c.querySelector('[data-slot="card-title"]')?.textContent.trim() === TITLE)`;
  const expect = {
    ok: { disks: 3, facts: 1, alerts: 0 },
    warning: { disks: 2, facts: 1, alerts: 2 },
    long: { disks: 3, facts: 1, alerts: 1 },
    unavailable: { disks: 0 },
    loading: { disks: 0 },
    failed: { disks: 0 },
    empty: { disks: 0 },
  };
  for (const lang of ["zh", "en"]) {
    const s = lang === "zh" ? zh : en;
    const notes = {
      unavailable: s["dh.windowsOnly"],
      loading: s["dh.loading"],
      empty: s["dh.none"],
      failed: "WMI provider is not available",
    };
    for (const state of Object.keys(expect))
      for (const scale of [1, 1.25, 1.5]) {
        const at = `${lang}/${state}/${scale}`;
        await send("Emulation.setDeviceMetricsOverride", {
          width: 1120,
          height: Math.round(1350 / scale),
          deviceScaleFactor: scale,
          mobile: false,
        });
        await send("Page.navigate", {
          url: `${base}/main-page-panel.html?lang=${lang}&state=${state}`,
        });
        if (
          !(await waitFor(
            "!!document.querySelector('#resource-trigger-storage')",
            15000,
          ))
        ) {
          problems.push(`${at}: the storage card never rendered`);
          continue;
        }
        const before = await evaluate(
          "({ calls: window.diskHealthCalls.length, timers: window.healthTimers() })",
        );
        if (before.calls || before.timers)
          problems.push(
            `${at}: disk health asked before the panel opened (${JSON.stringify(before)})`,
          );
        await click("#resource-trigger-storage");
        const found = summary.replace("TITLE", JSON.stringify(s["dh.title"]));
        if (!(await waitFor(`!!(${found})`))) {
          problems.push(
            `${at}: no Physical disks section in the storage panel`,
          );
          continue;
        }
        await sleep(700);
        await evaluate(`(${found}).scrollIntoView({ block: "start" })`);
        await sleep(200);
        const m = await evaluate(`(() => {
          const card = ${found};
          const sheet = document.querySelector('${SHEET}');
          const disks = [...card.querySelectorAll('.disk')];
          const edge = sheet.getBoundingClientRect().right;
          const clipped = [...card.querySelectorAll('.name, .status, [data-slot="card-title"], button, .facts')]
            .filter((e) => e.scrollWidth > e.clientWidth + 0.5 || e.getBoundingClientRect().right > edge + 0.5)
            .map((e) => e.textContent.trim());
          const overlap = disks.flatMap((d) => {
            const [a, b] = d.firstElementChild.children;
            return a.getBoundingClientRect().right > b.getBoundingClientRect().left + 0.5 ? [a.textContent.trim()] : [];
          });
          return {
            text: card.innerText,
            disks: disks.length,
            facts: disks.filter((d) => d.querySelector('.facts')).length,
            alerts: disks.filter((d) => d.querySelector('.status')?.className.includes('peach')).length,
            factsText: disks.map((d) => d.querySelector('.facts')?.textContent.trim() ?? null),
            capacity: sheet.innerText.includes('C:\\\\') && !!sheet.querySelector('[data-slot="progress"], [role="progressbar"]'),
            chart: !!sheet.querySelector('section'),
            calls: window.diskHealthCalls.length,
            timers: window.healthTimers(),
            clipped,
            overlap,
          };
        })()`);
        report.push({ lang, state, scale, ...m, text: undefined });
        const box = await evaluate(
          `(() => { const r = document.querySelector('${SHEET}').getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })()`,
        );
        const png = await send("Page.captureScreenshot", {
          format: "png",
          clip: { ...box, scale: 1 },
        });
        writeFileSync(
          `${out}/sidebar-${tag}-${lang}-${state}-${Math.round(scale * 100)}.png`,
          Buffer.from(png.result.data, "base64"),
        );
        for (const [k, v] of Object.entries(expect[state]))
          if (m[k] !== v) problems.push(`${at}: ${k} ${m[k]}, expected ${v}`);
        if (notes[state] && !m.text.includes(notes[state]))
          problems.push(`${at}: missing note ${notes[state]}`);
        if (!m.text.includes(s["dh.viewDetails"]))
          problems.push(`${at}: no health details link`);
        if (m.calls !== 1) problems.push(`${at}: ${m.calls} requests on open`);
        if (m.timers !== 1)
          problems.push(`${at}: ${m.timers} refresh timers while open`);
        if (!m.capacity || !m.chart)
          problems.push(
            `${at}: capacity, volumes or chart missing from the storage panel`,
          );
        const leak = /\b0 °C|undefined|NaN|null|\[object/;
        if (leak.test(m.text))
          problems.push(
            `${at}: placeholder or zero leaked: ${m.text.match(leak)[0]}`,
          );
        if (lang === "zh" && m.text.includes("。"))
          problems.push(`${at}: Chinese full stop`);
        if (m.clipped.length) problems.push(`${at}: clipped ${m.clipped[0]}`);
        if (m.overlap.length)
          problems.push(`${at}: name and status overlap at ${m.overlap[0]}`);
        if (
          state === "ok" &&
          m.factsText.join("|") !==
            `${s["dh.temp"]} 41 °C · ${s["dh.usedShort"]} 3%||`
        )
          problems.push(`${at}: facts ${JSON.stringify(m.factsText)}`);
        if (scale !== 1 || !["ok", "loading"].includes(state)) continue;
        if (state === "loading") {
          for (const type of ["keyDown", "keyUp"])
            await send("Input.dispatchKeyEvent", {
              type,
              key: "Escape",
              code: "Escape",
              windowsVirtualKeyCode: 27,
            });
          await waitFor(`!document.querySelector('${SHEET}')`);
          await sleep(300);
          const closed = await evaluate(
            "({ timers: window.healthTimers(), focus: document.activeElement?.id })",
          );
          report.push({ lang, step: "escape", ...closed });
          if (closed.timers)
            problems.push(
              `${at}: refresh timer still running after the panel closed`,
            );
          if (closed.focus !== "resource-trigger-storage")
            problems.push(`${at}: focus after Esc on ${closed.focus}`);
          continue;
        }
        await click(`${SHEET} button:has(.lucide-heart-pulse)`);
        const left = await waitFor(
          `!document.querySelector('${SHEET}') && !!document.getElementById('disk-health-title')`,
        );
        await sleep(400);
        const nav = await evaluate(`(() => ({
          left: !document.querySelector('${SHEET}'),
          tab: document.querySelector('nav.tabs button.active')?.textContent.trim(),
          detail: document.querySelector('main .head h2')?.textContent.trim(),
          focus: document.activeElement?.id,
          locked: getComputedStyle(document.body).pointerEvents === 'none' || document.body.style.overflow === 'hidden',
          inView: (() => { const r = document.getElementById('disk-health-title')?.getBoundingClientRect(); return !!r && r.top >= 0 && r.bottom <= innerHeight; })(),
          calls: window.diskHealthCalls.length,
          timers: window.healthTimers(),
        }))()`);
        report.push({ lang, step: "navigate", ...nav });
        await shot(`sidebar-${tag}-${lang}-after-link.png`);
        if (!left || !nav.left)
          problems.push(
            `${at}: the link did not close the panel and open the Disk detail`,
          );
        if (nav.tab !== s["tabs.dashboard"])
          problems.push(`${at}: tab after the link ${nav.tab}`);
        if (nav.detail !== s["detail.disk"])
          problems.push(`${at}: detail after the link ${nav.detail}`);
        if (nav.focus !== "disk-health-title" || !nav.inView)
          problems.push(
            `${at}: focus ${nav.focus}, health title in view ${nav.inView}`,
          );
        if (nav.locked)
          problems.push(`${at}: page still locked by the closed panel`);
        if (nav.timers !== 1)
          problems.push(
            `${at}: ${nav.timers} refresh timers on the Disk detail`,
          );
        await click("main .head .back");
        await waitFor("!document.querySelector('main .head .back')");
        await sleep(200);
        const back = await evaluate(
          "({ timers: window.healthTimers(), calls: window.diskHealthCalls.length })",
        );
        report.push({ lang, step: "back", ...back });
        if (back.timers)
          problems.push(
            `${at}: refresh timer still running after leaving the Disk detail`,
          );
      }
  }
  writeFileSync(`${out}/sidebar-${tag}.json`, JSON.stringify(report, null, 2));
  ws.close();
} finally {
  chrome.kill();
  await server.close();
}
console.log(`${report.length} checks`);
console.log(
  problems.length
    ? `FAIL\n${problems.join("\n")}`
    : "PASS: one entry per physical disk with only reported fields, every state explained, nothing clipped, the link closes the panel and focuses health in the Disk detail, no refresh timer once closed",
);
process.exitCode = problems.length ? 1 : 0;
