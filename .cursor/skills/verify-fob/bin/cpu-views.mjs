// cpu-views.mjs <zh|en> <evidence-dir>: serves the mocked CPU views (real
// Dashboard card, mini window card, process-tab CPU card, CPU detail and
// CPU details sheet, production CSS) for a hybrid Intel, a hybrid AMD, a
// uniform CPU, an unread layout and a missing reading, and fails unless each
// shows the split it should. Mocked readings only; this is not Windows evidence.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { createServer } from "vite";

const [lang = "zh", out = "/workspace/scratch/verify/evidence"] =
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
const port = 9310 + (lang === "zh" ? 1 : 2);
const W = 1400;
const chrome = spawn(
  "google-chrome",
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    "--hide-scrollbars",
    `--remote-debugging-port=${port}`,
    `--window-size=${W},900`,
    "about:blank",
  ],
  { stdio: "ignore" },
);
const L =
  lang === "zh"
    ? {
        p: "P 核",
        e: "E 核",
        perf: "性能核",
        eff: "能效核",
        intelSize: ["8 核 / 16 线程", "8 核 / 8 线程"],
        amdSize: ["4 核 / 8 线程", "8 核 / 16 线程"],
        groupsNote: "每组是组内线程的平均占用",
        perCoreNote: "每个核心按 0–100% 显示",
        uniformFoot: /^空闲 · 4 核$|^活跃 · 4 核$/,
      }
    : {
        p: "P-cores",
        e: "E-cores",
        perf: "Performance cores",
        eff: "Efficiency cores",
        intelSize: ["8 cores / 16 threads", "8 cores / 8 threads"],
        amdSize: ["4 cores / 8 threads", "8 cores / 16 threads"],
        groupsNote: "Each group is the average of its threads",
        perCoreNote: "Each core from 0 to 100%",
        uniformFoot: /^(idle|active) · 4 cores$/,
      };
const range = (a, b) => Array.from({ length: b - a }, (_, i) => a + i);
const load = (i, busy) => (busy ? 62 + ((i * 37) % 30) : 4 + ((i * 13) % 9));
const avg = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;
const groupPct = (threads, busy) =>
  String(Math.round(avg(threads.map((i) => load(i, busy(i))))));

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
  await send("Emulation.setDeviceMetricsOverride", {
    width: W,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false,
  });

  const read = () =>
    evaluate(`(() => {
    const q = (root, sel) => [...root.querySelectorAll(sel)];
    const text = (el) => el?.textContent.replace(/\\s+/g, ' ').trim() ?? null;
    const fits = (el) => !el || el.scrollWidth <= el.clientWidth + 1;
    const dashCard = (id) => {
      const card = q(document.querySelector(id), '.grid .card')[0];
      const foot = card.querySelector('.foot');
      return {
        big: text(card.querySelector('.big')),
        foot: text(foot),
        footFits: fits(foot),
        groups: q(card, '.bar-group').map((g) => g.querySelectorAll('.bar').length),
        bars: q(card, '.bar').length,
        titles: q(card, '.bar').slice(0, 1).map((b) => b.title),
      };
    };
    const statCard = (id) => {
      const root = document.querySelector(id);
      const middle = root.querySelector('.sc-middle').getBoundingClientRect();
      const strips = q(root, '.strip-group');
      return {
        value: text(root.querySelector('.sc-value')),
        strips: strips.map((g) => ({
          text: [...g.querySelector('.line').children].map(text).join(' '),
          cells: g.querySelectorAll('.heat').length,
          title: g.title,
        })),
        inside: strips.every((g) => g.getBoundingClientRect().bottom <= middle.bottom + 0.5),
        labelsFit: strips.every((g) => fits(g.querySelector('.truncate'))),
        flatCells: q(root, '.sc-middle .cell, .sc-middle .cores .heat').length,
      };
    };
    const detail = document.querySelector('#detail');
    const resource = document.querySelector('#resource');
    return {
      dashboard: dashCard('#dashboard'),
      mini: dashCard('#mini'),
      narrow: statCard('#card-narrow'),
      wide: statCard('#card-wide'),
      miniDetail: (() => {
        const el = document.querySelector('#mini-detail');
        const section = el.querySelector('section.detail');
        return {
          heads: q(el, '.group-head').map(text),
          height: section.getBoundingClientRect().height,
          // 652 px window less padding (8 + 10), title bar (28) and its gap (6).
          fits: section.getBoundingClientRect().height <= 600,
        };
      })(),
      detail: {
        big: text(detail.querySelector('.big')),
        heads: q(detail, '.group-head').map(text),
        cells: q(detail, '.cell').length,
      },
      resource: {
        text: text(resource),
        heads: q(resource, '.group-head').map(text),
        cells: q(resource, '.cell').length,
      },
    };
  })()`);

  const reports = {};
  for (const cpu of ["intel", "amd", "uniform", "unknown", "noreading"]) {
    await send("Page.navigate", {
      url: `${base}/cpu-views-panel.html?lang=${lang}&cpu=${cpu}`,
    });
    for (
      let i = 0;
      i < 100 &&
      !(await evaluate("!!document.querySelector('#resource .cell')"));
      i++
    )
      await new Promise((r) => setTimeout(r, 100));
    await new Promise((r) => setTimeout(r, 300));
    const r = await read();
    reports[cpu] = r;
    const { width, height } = await evaluate(
      "({ width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight })",
    );
    writeFileSync(
      `${out}/cpu-views-${lang}-${cpu}.png`,
      Buffer.from(
        (
          await send("Page.captureScreenshot", {
            format: "png",
            captureBeyondViewport: true,
            clip: { x: 0, y: 0, width, height, scale: 1 },
          })
        ).result.data,
        "base64",
      ),
    );

    for (const section of [
      "dashboard",
      "mini",
      "cards",
      "detail",
      "mini-detail",
      "resource",
    ]) {
      const sel =
        section === "dashboard" || section === "mini"
          ? `#${section} .grid .card`
          : `#${section}`;
      const clip = await evaluate(
        `(() => { const r = document.querySelector(${JSON.stringify(sel)}).getBoundingClientRect(); return { x: r.x, y: r.y + scrollY, width: r.width, height: r.height, scale: 2 }; })()`,
      );
      writeFileSync(
        `${out}/cpu-views-${lang}-${cpu}-${section}.png`,
        Buffer.from(
          (
            await send("Page.captureScreenshot", {
              format: "png",
              captureBeyondViewport: true,
              clip,
            })
          ).result.data,
          "base64",
        ),
      );
    }
    const all = [
      r.dashboard.foot,
      r.mini.foot,
      r.resource.text,
      ...r.detail.heads,
      ...r.miniDetail.heads,
    ];
    if (cpu === "intel" || cpu === "amd") {
      const [pName, eName] = cpu === "intel" ? [L.p, L.e] : [L.perf, L.eff];
      const pCount = cpu === "intel" ? 16 : 8;
      const busy = (i) => i < pCount;
      const pPct = groupPct(range(0, pCount), busy);
      const ePct = groupPct(range(pCount, 24), busy);
      const exact = avg(range(0, 24).map((i) => load(i, busy(i))));
      const total = String(Math.round(exact));
      const d = r.dashboard;
      assert.equal(
        d.foot,
        `${pName} ${pPct}% · ${eName} ${ePct}%`,
        `${cpu} dashboard foot`,
      );
      assert.ok(
        d.footFits,
        `${cpu} dashboard: the split fits without an ellipsis`,
      );
      assert.deepEqual(
        d.groups,
        [pCount, 24 - pCount],
        `${cpu} dashboard: bars grouped`,
      );
      for (const where of ["dashboard", "mini"])
        assert.ok(
          r[where].big.startsWith(total),
          `${cpu} ${where}: total stays the all-thread average`,
        );
      // The mini card keeps its one-line footer; its CPU detail has the split.
      assert.deepEqual(r.mini.groups, [], `${cpu} mini card unchanged`);
      assert.ok(r.mini.footFits, `${cpu} mini footer fits`);
      assert.deepEqual(
        r.miniDetail.heads,
        r.detail.heads,
        `${cpu} mini detail split`,
      );
      assert.ok(
        r.miniDetail.fits,
        `${cpu} mini detail fits the mini window height`,
      );
      for (const where of ["narrow", "wide"]) {
        assert.deepEqual(
          r[where].strips.map((s) => [s.text, s.cells]),
          [
            [`${pName} ${pPct}%`, pCount],
            [`${eName} ${ePct}%`, 24 - pCount],
          ],
          `${cpu} ${where} card groups`,
        );
        assert.ok(
          r[where].inside,
          `${cpu} ${where}: groups fit the card's core area`,
        );
        assert.ok(
          r[where].value.startsWith(exact.toFixed(1)),
          `${cpu} ${where}: card total unchanged`,
        );
      }
      assert.ok(r.wide.labelsFit, `${cpu} wide card: group names not cut`);
      const size = cpu === "intel" ? L.intelSize : L.amdSize;
      assert.deepEqual(r.detail.heads, [
        `${pName} ${pPct}% ${size[0]}`,
        `${eName} ${ePct}% ${size[1]}`,
      ]);
      assert.deepEqual(r.resource.heads, r.detail.heads);
      assert.equal(r.detail.cells, 24, "every thread still has its bar");
      assert.ok(r.resource.text.includes(L.groupsNote));
      if (cpu === "amd")
        assert.ok(
          !all.some((s) => s.includes(L.p) || s.includes(L.e)),
          "AMD never says P / E",
        );
    } else if (cpu === "noreading") {
      const pPct = groupPct(range(0, 16), () => true);
      assert.equal(r.dashboard.foot, `${L.p} ${pPct}% · ${L.e} —`);
      assert.ok(r.miniDetail.heads[1].startsWith(`${L.e} —`));
      assert.deepEqual(
        r.narrow.strips.map((s) => s.text),
        [`${L.p} ${pPct}%`, `${L.e} —`],
      );
      assert.ok(
        r.detail.heads[1].startsWith(`${L.e} —`),
        "no reading is a dash, not 0%",
      );
      assert.ok(!all.some((s) => s.includes(`${L.e} 0%`)));
    } else {
      // No split: today's views, no headings, no empty groups.
      for (const where of ["dashboard", "mini"]) {
        assert.deepEqual(r[where].groups, [], `${cpu} ${where}: no groups`);
        const n = cpu === "uniform" ? 8 : 24;
        assert.equal(r[where].bars, n);
        if (cpu === "uniform") assert.match(r[where].foot, L.uniformFoot);
      }
      for (const where of ["narrow", "wide"]) {
        assert.deepEqual(
          r[where].strips,
          [],
          `${cpu} ${where}: no group strips`,
        );
        assert.ok(
          r[where].flatCells > 0,
          `${cpu} ${where}: per-core grid kept`,
        );
      }
      assert.deepEqual(r.detail.heads, []);
      assert.deepEqual(r.miniDetail.heads, []);
      assert.deepEqual(r.resource.heads, []);
      assert.ok(r.resource.text.includes(L.perCoreNote));
      for (const word of [L.p, L.e, L.perf, L.eff])
        assert.ok(!all.some((s) => s.includes(word)), `${cpu}: no "${word}"`);
    }
  }
  writeFileSync(
    `${out}/cpu-views-${lang}.json`,
    JSON.stringify(reports, null, 2),
  );
  console.log(
    `PASS ${lang}: Intel P/E, AMD performance/efficiency, uniform and unread show no split, missing reading is a dash; mini foot ${reports.intel.mini.foot} | ${reports.amd.mini.foot}`,
  );
  ws.close();
} finally {
  chrome.kill();
  await server.close();
}
