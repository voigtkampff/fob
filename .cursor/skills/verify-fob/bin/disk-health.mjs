// disk-health.mjs <evidence-dir> [tag]: renders the real Disk detail
// (CardDetail + DiskHealth, i18n and production CSS) with generic mocked
// disk_health replies in every state, zh and en, at device scale 1, 1.25
// and 1.5. Checks the panel is in the main window's Disk detail and not the
// mini window, that opening it sends one request, that every state is
// explained, and that nothing is clipped or overlaps. Writes
// disk-health-<tag>-<lang>-<state>-<scale>.png and disk-health-<tag>.json.
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
const port = 9311;
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
  const STATES = [
    "ok",
    "warning",
    "errors",
    "empty",
    "unavailable",
    "pending",
    "loading",
    "failed",
  ];
  const expect = {
    ok: { disks: 3, alerts: 0, warnings: 0 },
    warning: { disks: 2, alerts: 2, warnings: 1 },
    errors: { disks: 3, alerts: 0, warnings: 0 },
    empty: { disks: 0 },
    unavailable: { disks: 0 },
    pending: { disks: 0 },
    loading: { disks: 0 },
    failed: { disks: 0 },
  };
  const notes = {
    en: {
      empty: en["dh.none"],
      unavailable: en["dh.windowsOnly"],
      pending: en["dh.loading"],
      loading: en["dh.loading"],
      failed: "WMI provider is not available",
    },
    zh: {
      empty: zh["dh.none"],
      unavailable: zh["dh.windowsOnly"],
      pending: zh["dh.loading"],
      loading: zh["dh.loading"],
      failed: "WMI provider is not available",
    },
  };
  for (const lang of ["zh", "en"])
    for (const state of STATES)
      for (const scale of [1, 1.25, 1.5]) {
        await send("Emulation.setDeviceMetricsOverride", {
          width: 1440,
          height: 1400,
          deviceScaleFactor: scale,
          mobile: false,
        });
        await send("Page.navigate", {
          url: `${base}/disk-health-panel.html?lang=${lang}&state=${state}`,
        });
        for (
          let i = 0;
          i < 100 &&
          !(await evaluate("!!document.querySelector('#detail .health')"));
          i++
        )
          await new Promise((r) => setTimeout(r, 100));
        await new Promise((r) => setTimeout(r, 1200));
        const m = await evaluate(`(() => {
          const panel = document.querySelector('#detail .health');
          const text = panel?.innerText ?? '';
          const clipped = [...(panel?.querySelectorAll('dt, dd, .note, .tag, .name, .warnings span') ?? [])]
            .filter((e) => e.scrollWidth > e.clientWidth + 0.5 && getComputedStyle(e).overflow !== 'hidden')
            .map((e) => e.textContent.trim());
          const overlap = [...(panel?.querySelectorAll('dl') ?? [])].flatMap((dl) => {
            const kids = [...dl.children];
            const out = [];
            for (let i = 0; i + 1 < kids.length; i += 2) {
              const a = kids[i].getBoundingClientRect(), b = kids[i + 1].getBoundingClientRect();
              if (b.left < a.right - 0.5 && Math.abs(a.top - b.top) < 2) out.push(kids[i].textContent.trim());
            }
            return out;
          });
          const panelBox = panel?.getBoundingClientRect();
          const outside = [...(panel?.querySelectorAll('*') ?? [])].filter((e) => e.getBoundingClientRect().right > panelBox.right + 0.5).map((e) => e.textContent.trim().slice(0, 40));
          return {
            present: !!panel,
            inMini: !!document.querySelector('#mini .health'),
            calls: window.diskHealthCalls.length,
            disks: panel?.querySelectorAll('.disk').length ?? 0,
            alerts: panel?.querySelectorAll('.disk.alert').length ?? 0,
            warnings: panel?.querySelectorAll('.warnings').length ?? 0,
            text,
            clipped,
            overlap,
            outside,
          };
        })()`);
        const at = `${lang}/${state}/${scale}`;
        report.push({ lang, state, scale, ...m, text: undefined });
        const box = await evaluate(
          "(() => { const r = document.querySelector('#detail').getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })()",
        );
        if (scale === 1 || ["ok", "warning", "errors"].includes(state)) {
          const shot = await send("Page.captureScreenshot", {
            format: "png",
            clip: { ...box, scale: 1 },
          });
          writeFileSync(
            `${out}/disk-health-${tag}-${lang}-${state}-${Math.round(scale * 100)}.png`,
            Buffer.from(shot.result.data, "base64"),
          );
        }
        if (!m.present)
          problems.push(`${at}: no health panel in the Disk detail`);
        if (m.inMini)
          problems.push(`${at}: health panel shown in the mini window`);
        if (m.calls !== 1)
          problems.push(`${at}: disk_health called ${m.calls} times on open`);
        for (const [k, v] of Object.entries(expect[state]))
          if (m[k] !== v) problems.push(`${at}: ${k} ${m[k]}, expected ${v}`);
        if (notes[lang][state] && !m.text.includes(notes[lang][state]))
          problems.push(`${at}: missing note ${notes[lang][state]}`);
        if (lang === "zh" && m.text.includes("。"))
          problems.push(`${at}: Chinese full stop in visible text`);
        if (/undefined|NaN|null|\[object/.test(m.text))
          problems.push(
            `${at}: placeholder text leaked: ${m.text.match(/undefined|NaN|null|\[object/)[0]}`,
          );
        if (
          state === "errors" &&
          !m.text.includes(
            lang === "zh" ? zh["dh.nvme.denied"] : en["dh.nvme.denied"],
          )
        )
          problems.push(`${at}: denied state not explained`);
        if (m.clipped.length) problems.push(`${at}: clipped ${m.clipped[0]}`);
        if (m.overlap.length)
          problems.push(`${at}: label and value overlap at ${m.overlap[0]}`);
        if (m.outside.length)
          problems.push(`${at}: content past the panel edge: ${m.outside[0]}`);
      }
  writeFileSync(
    `${out}/disk-health-${tag}.json`,
    JSON.stringify(report, null, 2),
  );
  ws.close();
} finally {
  chrome.kill();
  await server.close();
}
console.log(`${report.length} renders`);
console.log(
  problems.length
    ? `FAIL\n${problems.join("\n")}`
    : "PASS: health panel in the Disk detail only, one request on open, every state explained, nothing clipped or overlapping, no Chinese full stop",
);
process.exitCode = problems.length ? 1 : 0;
