// disk-health-hover.mjs <evidence-dir> [tag]: hovers every metric row of the
// real Disk detail (production CSS, generic mocked disk_health replies) with
// a CDP pointer on the label, the gap and the value, then leaves. Each hover
// must paint only that row's label and value chips, keep every box in place,
// keep text at 4.5:1 or more, ring warning values, and clear on leave. Runs
// every theme on the warning fixture, then zh and en, four states, panel
// widths 980 (the app's cap), 560 and 300 (narrower than the app allows,
// where values wrap) and emulated device scales 1 and 1.5 (browser emulation,
// not system DPI) in one dark and one light theme. Mocked readings only; this is not
// Windows evidence.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { themes } from "../../../../src/lib/definitions/themes.ts";
import { checkDiskHover } from "../../../../scripts/package-smoke/hover.mjs";

const [out = "/workspace/scratch/verify/evidence", tag = "hover"] =
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
const port = 9313;
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
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const problems = [];
const report = [];
try {
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await sleep(200);
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
    ).result.result?.value;
  const pointTo = (x, y) =>
    send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  const open = async ({ theme, lang, state, width, scale }) => {
    await send("Emulation.setDeviceMetricsOverride", {
      width: width + 480,
      height: 1800,
      deviceScaleFactor: scale,
      mobile: false,
    });
    await send("Page.navigate", { url: `${base}/disk-health-panel.html` });
    await sleep(300);
    await evaluate(`localStorage.setItem('theme', ${JSON.stringify(theme)})`);
    await send("Page.navigate", {
      url: `${base}/disk-health-panel.html?lang=${lang}&state=${state}&width=${width}`,
    });
    for (
      let i = 0;
      i < 100 &&
      !(await evaluate("!!document.querySelector('#detail .health dl')"));
      i++
    )
      await sleep(100);
    await sleep(500);
    return evaluate("document.documentElement.dataset.theme");
  };
  const frame = () =>
    evaluate(
      "new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))",
    );
  const shot = async (name) => {
    await frame();
    const box = await evaluate(
      "(() => { const r = document.querySelector('#detail .health').getBoundingClientRect(); return { x: r.x + scrollX - 4, y: r.y + scrollY - 4, width: r.width + 8, height: r.height + 8 }; })()",
    );
    const png = await send("Page.captureScreenshot", {
      format: "png",
      clip: { ...box, scale: 1 },
    });
    writeFileSync(`${out}/${name}`, Buffer.from(png.result.data, "base64"));
  };
  const runs = [
    ...Object.keys(themes).map((theme) => ({
      theme,
      lang: "en",
      state: "warning",
      width: 980,
      scale: 1,
    })),
    ...["catppuccin", "githubLight"].flatMap((theme) =>
      ["zh", "en"].flatMap((lang) =>
        ["ok", "warning", "errors", "long"].flatMap((state) =>
          [
            [980, 1],
            [980, 1.5],
            [560, 1],
            [300, 1],
          ].map(([width, scale]) => ({ theme, lang, state, width, scale })),
        ),
      ),
    ),
  ];
  for (const run of runs) {
    const at = `${run.theme}/${run.lang}/${run.state}/${run.width}px/x${run.scale}`;
    const applied = await open(run);
    if (applied !== run.theme) problems.push(`${at}: theme ${applied} applied`);
    const r = await checkDiskHover({ evaluate, pointTo, settle: frame });
    report.push({ ...run, ...r });
    if (!r.count) problems.push(`${at}: no metric rows`);
    if (
      run.state === "long" &&
      run.lang === "en" &&
      run.width === 300 &&
      !r.results.some((x) => x.wrapped && x.pass)
    )
      problems.push(`${at}: no wrapped value was hovered`);
    for (const f of r.results.filter((x) => x.msg))
      problems.push(`${at}: row ${f.row} ${f.msg}`);
    if (run.scale === 1) {
      const pick = r.results.findIndex((x) => x.warning);
      const row = pick >= 0 ? pick : Math.min(4, r.count - 1);
      const info = await evaluate(`window.__diskHover.row(${row})`);
      await pointTo(...info.points.gap);
      await sleep(150);
      await shot(
        `disk-health-hover-${tag}-${run.theme}-${run.lang}-${run.state}-${run.width}.png`,
      );
      await pointTo(...info.outside);
    }
  }
  writeFileSync(
    `${out}/disk-health-hover-${tag}.json`,
    JSON.stringify(report, null, 2),
  );
  const rows = report.reduce((n, r) => n + r.count, 0);
  console.log(`${report.length} renders, ${rows} rows hovered three ways`);
  ws.close();
} finally {
  chrome.kill();
  await server.close();
}
console.log(
  problems.length
    ? `FAIL\n${problems.join("\n")}`
    : "PASS: each hover paints only its row's label and value chips, nothing moves, text keeps 4.5:1, warnings keep a ring, chips clear on leave, no pointer cursor or tab stop",
);
process.exitCode = problems.length ? 1 : 0;
