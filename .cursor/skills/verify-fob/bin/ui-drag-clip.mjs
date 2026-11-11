// ui-drag-clip.mjs <evidence-dir>: lift a Processes or Dashboard summary card
// (and a column header) and drag it toward each edge of the window, at 1280
// and 1920 px and with the page scrolled. The lifted item must stay drawn in
// full wherever it is inside the window: no ancestor may cut it off, and it
// must be the top element under its own centre.
// Mocked readings in headless Chrome only; this is not Windows evidence.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
const [out = "/workspace/scratch/verify/evidence/ui-drag-clip"] =
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
const port = 9317;
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
const results = [];
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
  const evaluate = async (expression) => {
    const r = await send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (r.result.exceptionDetails)
      throw new Error(
        `${expression.slice(0, 80)}: ${r.result.exceptionDetails.exception?.description}`,
      );
    return r.result.result.value;
  };
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
  const mouse = (type, x, y, buttons = 0) =>
    send("Input.dispatchMouseEvent", {
      type,
      x,
      y,
      button: type === "mouseMoved" ? (buttons ? "left" : "none") : "left",
      buttons,
      clickCount: 1,
    });
  const center = (expr) =>
    evaluate(
      `(() => { const e = ${expr}; if (!e) return null; const r = e.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + Math.min(r.height / 2, 14), left: r.left, right: r.right }; })()`,
    );
  // Press, travel in steps, then (unless `hold`) release.
  const drag = async (from, to, { hold = false, steps = 6 } = {}) => {
    await mouse("mouseMoved", from.x, from.y);
    await mouse("mousePressed", from.x, from.y, 1);
    for (let i = 1; i <= steps; i++)
      await mouse(
        "mouseMoved",
        from.x + ((to.x - from.x) * i) / steps,
        from.y + ((to.y - from.y) * i) / steps,
        1,
      );
    if (!hold) await mouse("mouseReleased", to.x, to.y);
    await sleep(150);
  };
  const press = async (key, code, modifiers = 0) => {
    const keyCode = { ArrowRight: 39, ArrowLeft: 37, Escape: 27 }[key];
    for (const type of ["rawKeyDown", "keyUp"])
      await send("Input.dispatchKeyEvent", {
        type,
        key,
        code,
        modifiers,
        windowsVirtualKeyCode: keyCode,
      });
    await sleep(150);
  };
  const check = (ok, what) => {
    if (!ok) problems.push(what);
    return ok;
  };
  const viewport = (width, height) =>
    send("Emulation.setDeviceMetricsOverride", {
      width,
      height,
      deviceScaleFactor: 1,
      mobile: false,
    });
  const open = async (lang, tab) => {
    await send("Page.navigate", {
      url: `${base}/main-page-panel.html?lang=${lang}&procs&saved`,
    });
    await waitFor("!!document.querySelector('nav.tabs button')", 15000);
    await evaluate(
      `document.querySelectorAll('nav.tabs button')[${tab === "processes" ? 1 : 0}].click()`,
    );
    await waitFor(
      tab === "processes"
        ? "document.querySelectorAll('tbody tr').length >= 5 && !!document.querySelector('.resource-grid')"
        : "document.querySelectorAll('.card-head').length >= 4",
      15000,
    );
    await sleep(300);
  };
  // The part of the lifted item that is drawn: its box cut by the window and
  // by every ancestor that clips its overflow.
  const lifted = () =>
    evaluate(`(() => {
      const el = document.querySelector('[data-dragging]');
      if (!el) return null;
      const r = el.getBoundingClientRect();
      let clip = { left: 0, top: 0, right: innerWidth, bottom: innerHeight };
      const clippers = [];
      for (let a = el.parentElement; a; a = a.parentElement) {
        if (a.matches(':popover-open')) break;
        const cs = getComputedStyle(a);
        if (cs.overflowX === 'visible' && cs.overflowY === 'visible' && cs.contain.indexOf('paint') < 0) continue;
        const b = a.getBoundingClientRect();
        const box = { left: b.left + a.clientLeft, top: b.top + a.clientTop, right: b.left + a.clientLeft + a.clientWidth, bottom: b.top + a.clientTop + a.clientHeight };
        if (cs.overflowX === 'visible') { box.left = -1e9; box.right = 1e9; }
        if (cs.overflowY === 'visible') { box.top = -1e9; box.bottom = 1e9; }
        const cuts = r.left < box.left - 0.5 || r.top < box.top - 0.5 || r.right > box.right + 0.5 || r.bottom > box.bottom + 0.5;
        if (cuts) clippers.push((a.className && typeof a.className === 'string' ? a.tagName.toLowerCase() + '.' + a.className.trim().split(/\\s+/).slice(0, 3).join('.') : a.tagName.toLowerCase()) + ' ' + cs.overflowX + '/' + cs.overflowY);
        clip = { left: Math.max(clip.left, box.left), top: Math.max(clip.top, box.top), right: Math.min(clip.right, box.right), bottom: Math.min(clip.bottom, box.bottom) };
      }
      const inWin = { left: Math.max(r.left, 0), top: Math.max(r.top, 0), right: Math.min(r.right, innerWidth), bottom: Math.min(r.bottom, innerHeight) };
      const area = (b) => Math.max(0, b.right - b.left) * Math.max(0, b.bottom - b.top);
      const drawn = { left: Math.max(inWin.left, clip.left), top: Math.max(inWin.top, clip.top), right: Math.min(inWin.right, clip.right), bottom: Math.min(inWin.bottom, clip.bottom) };
      const cx = (inWin.left + inWin.right) / 2, cy = (inWin.top + inWin.bottom) / 2;
      const top = document.elementFromPoint(cx, cy);
      return {
        rect: [r.left, r.top, r.width, r.height].map(Math.round),
        shown: Math.round((100 * area(drawn)) / Math.max(1, area(inWin))),
        inTopLayer: !!el.closest(':popover-open'),
        onTop: !!el.closest(':popover-open') || !!top && (el.contains(top) || top === el || top.closest('[data-dragging]') === el),
        topAtCentre: top ? top.tagName.toLowerCase() + '.' + String(top.className).trim().split(/\\s+/).slice(0, 2).join('.') : null,
        clippers,
      };
    })()`);
  const lang = "zh";

  const probe = async (where, handleExpr, scroll = 0) => {
    const from = await center(handleExpr);
    const W = await evaluate("innerWidth");
    const H = await evaluate("innerHeight");
    const edges = {
      top: { x: from.x, y: 4 },
      bottom: { x: from.x, y: H - 4 },
      left: { x: 4, y: from.y },
      right: { x: W - 4, y: from.y },
    };
    for (const [edge, to] of Object.entries(edges)) {
      await drag(from, to, { hold: true, steps: 8 });
      await sleep(200);
      const l = await lifted();
      const name = `${where}-${edge}`;
      results.push({ name, ...l });
      if (!l) check(false, `${name}: nothing lifted`);
      else {
        check(
          l.shown >= 99,
          `${name}: only ${l.shown}% of the lifted item drawn, cut by ${l.clippers.join(" | ")}`,
        );
        check(
          !l.inTopLayer ||
            (l.rect[0] >= 0 &&
              l.rect[1] >= 0 &&
              l.rect[0] + l.rect[2] <= W + 1 &&
              l.rect[1] + l.rect[3] <= H + 1),
          `${name}: lifted card drawn outside the window ${l.rect}`,
        );
        check(l.onTop, `${name}: lifted item under ${l.topAtCentre}`);
      }
      await shot(`${name}.png`);
      await press("Escape", "Escape");
      await mouse("mouseReleased", to.x, to.y);
      await sleep(250);
      check(
        !(await evaluate("!!document.querySelector('[data-dragging]')")),
        `${name}: still lifted after Escape`,
      );
    }
  };
  for (const [w, h] of [
    [1280, 800],
    [1920, 1080],
  ]) {
    await viewport(w, h);
    await send("Page.navigate", { url: `${base}/main-page-panel.html` });
    await evaluate("sessionStorage.clear()");
    await open(lang, "processes");
    const pcard = (id) =>
      `document.querySelector('#resource-trigger-${id}').closest('.resource-grid > *').querySelector('.sc-title')`;
    await probe(`${w}-proc-card-first`, pcard("cpu"));
    await probe(
      `${w}-proc-card-last`,
      `[...document.querySelector('.resource-grid').children].at(-1).querySelector('.sc-title')`,
    );
    await probe(
      `${w}-header`,
      `document.querySelectorAll('thead tr th')[3].querySelector('button')`,
    );
    await evaluate("document.querySelectorAll('nav.tabs button')[0].click()");
    await waitFor("document.querySelectorAll('.card-head').length >= 4");
    await sleep(300);
    const grid =
      "document.querySelector('.card-head').closest('.card').parentElement";
    await probe(
      `${w}-dash-first`,
      `${grid}.children[0].querySelector('.card-head .drag-grip')`,
    );
    await probe(
      `${w}-dash-last`,
      `${grid}.lastElementChild.querySelector('.card-head .drag-grip')`,
    );
    const scrolled = await evaluate(
      `(() => { let a = ${grid}; while (a && a.scrollHeight <= a.clientHeight + 1) a = a.parentElement; if (!a) return 0; a.scrollTop = 120; return a.scrollTop; })()`,
    );
    if (scrolled)
      await probe(
        `${w}-dash-scrolled`,
        `${grid}.children[1].querySelector('.card-head .drag-grip')`,
      );
    results.push({ name: `${w}-dash-scrollTop`, scrolled });
  }
  ws.close();
} catch (error) {
  problems.push(`run failed: ${error.stack ?? error}`);
} finally {
  chrome.kill();
  await server.close();
}
writeFileSync(
  `${out}/ui-drag-clip.json`,
  JSON.stringify({ problems, results }, null, 2),
);
for (const r of results) console.log(JSON.stringify(r));
if (problems.length) {
  console.error(`FAIL\n${problems.join("\n")}`);
  process.exit(1);
}
console.log(`PASS ui-drag-clip -> ${out}`);
