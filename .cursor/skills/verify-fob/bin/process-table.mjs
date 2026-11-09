// process-table.mjs <zh|en> <evidence-dir>: serves the mocked process-table
// preview (real ProcessTable, columns, sort, i18n and production CSS) and
// drives it in headless Chrome with real pointer events. Mocked readings only;
// this is not Windows evidence.
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
const port = 9300 + (lang === "zh" ? 1 : 2);
const chrome = spawn(
  "google-chrome",
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    "--hide-scrollbars",
    `--remote-debugging-port=${port}`,
    "--window-size=1280,520",
    "about:blank",
  ],
  { stdio: "ignore" },
);
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
    width: 1280,
    height: 520,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await send("Page.navigate", {
    url: `${base}/process-table-panel.html?lang=${lang}`,
  });
  for (
    let i = 0;
    i < 100 &&
    !(await evaluate("document.querySelectorAll('tbody tr').length"));
    i++
  )
    await new Promise((r) => setTimeout(r, 100));
  const table = () =>
    evaluate(`(() => {
    const heads = [...document.querySelectorAll('thead th')].map((th) => th.textContent.trim());
    const rows = [...document.querySelectorAll('tbody tr')].map((tr) => [...tr.children].map((td) => ({ text: td.textContent.trim(), title: td.querySelector('[title]')?.getAttribute('title') ?? null })));
    const region = document.querySelector('.process-table-region');
    return { heads, rows, scrollWidth: region.scrollWidth, clientWidth: region.clientWidth };
  })()`);
  const click = async (label) => {
    const box = await evaluate(
      `(() => { const b = [...document.querySelectorAll('thead th button')].find((b) => b.textContent.trim() === ${JSON.stringify(label)}); const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
    );
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
      await send("Input.dispatchMouseEvent", {
        type,
        x: box.x,
        y: box.y,
        button: "left",
        clickCount: 1,
      });
    await new Promise((r) => setTimeout(r, 300));
  };
  const shot = async (name) =>
    writeFileSync(
      `${out}/${name}`,
      Buffer.from(
        (await send("Page.captureScreenshot", { format: "png" })).result.data,
        "base64",
      ),
    );
  const L =
    lang === "zh"
      ? { read: "硬盘读取", write: "硬盘写入", gpu: "GPU %" }
      : { read: "Disk read", write: "Disk write", gpu: "GPU %" };
  const before = await table();
  const col = (t, label) => t.heads.indexOf(label);
  const names = (t) => t.rows.map((r) => r[0].text);
  const report = {
    lang,
    heads: before.heads,
    scrollWidth: before.scrollWidth,
    clientWidth: before.clientWidth,
  };
  await shot(`process-table-${lang}.png`);
  await click(L.read);
  const byRead = await table();
  report.byRead = byRead.rows.map((r) => [
    r[0].text,
    r[col(byRead, L.read)].text,
  ]);
  assert.deepEqual(
    names(byRead).slice(0, 2),
    ["MsMpEng.exe", "Game.exe"],
    "busiest reader first",
  );
  assert.deepEqual(
    names(byRead).slice(-2).sort(),
    ["System", "setup.exe"],
    "no reading last",
  );
  const dash = byRead.rows.find((r) => r[0].text === "System")[
    col(byRead, L.read)
  ];
  const idle = byRead.rows.find((r) => r[0].text === "explorer.exe")[
    col(byRead, L.read)
  ];
  assert.equal(dash.text, "—");
  assert.equal(idle.text, "0 B/s");
  assert.notEqual(dash.title, idle.title, "no reading has its own tooltip");
  report.noReadingTitle = dash.title;
  await shot(`process-table-${lang}-sorted-disk-read.png`);
  await click(L.read);
  const asc = await table();
  assert.deepEqual(
    names(asc).slice(-2).sort(),
    ["System", "setup.exe"],
    "no reading last ascending too",
  );
  await click(L.gpu);
  const byGpu = await table();
  report.byGpu = byGpu.rows.map((r) => [r[0].text, r[col(byGpu, L.gpu)].text]);
  assert.equal(names(byGpu)[0], "Game.exe");
  await click(L.write);
  const byWrite = await table();
  report.byWrite = byWrite.rows.map((r) => [
    r[0].text,
    r[col(byWrite, L.write)].text,
  ]);
  assert.deepEqual(names(byWrite).slice(0, 2), ["OneDrive.exe", "chrome.exe"]);
  writeFileSync(
    `${out}/process-table-${lang}.json`,
    JSON.stringify(report, null, 2),
  );
  console.log(
    `PASS ${lang}: pointer sort by disk read/write and GPU, dash vs 0 B/s, no reading last; scroll ${before.scrollWidth}/${before.clientWidth}`,
  );
  ws.close();
} finally {
  chrome.kill();
  await server.close();
}
