// ui-reorder.mjs <evidence-dir>: the real main window route (+page.svelte,
// production CSS) with generic mocked processes and readings. With real
// pointer and key events, in zh and en, it checks:
// - process column headers drag into a new order, header and body cells move
//   together, the name column stays first, the actions column takes no drop,
//   a short click still sorts, Escape cancels without sorting, Alt+Arrow
//   moves the focused header, hidden columns and tree view keep their place;
// - while a header or card is dragged it lifts and follows the pointer, the
//   others already stand in the order it would land in (header and body
//   cells together), and nothing is saved until release; with reduced
//   motion they jump instead of sliding;
// - movable headers and every summary card show a dot grip (none on the name
//   and actions columns), stronger on hover, with a short drag hint in the
//   tooltip, and a Dashboard card drags from its grip;
// - Processes and Dashboard summary cards drag by their title, the drag opens
//   nothing and the next click still does;
// - all three orders survive a reload, and bad saved orders (unknown,
//   repeated, missing ids) still show every card and column once;
// - at 1920 px the name column stops at its widest, the other columns share
//   the rest evenly and the actions column ends at the right edge; at 1024
//   px the name column is at its narrowest and the table scrolls sideways.
// Mocked readings in headless Chrome only; this is not Windows evidence.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { en, zh } from "../../../../src/lib/i18n/strings.js";
import {
  columnWidth,
  nameMaxWidth,
  nameMinWidth,
} from "../../../../src/lib/utils/table-layout.js";

const idOf = (strings, label) =>
  ({
    [strings["col.familyCpu"]]: "cpu_usage",
    [strings["col.familyRam"]]: "memory_usage",
  })[label] ??
  Object.keys(strings)
    .find((k) => k.startsWith("col.") && strings[k] === label)
    ?.slice(4) ??
  label;

const [out = "/workspace/scratch/verify/evidence/ui-reorder"] =
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
  const saved = () =>
    evaluate(
      "JSON.parse(sessionStorage.getItem('preview-config') ?? 'null')?.behavior ?? {}",
    );
  const editSaved = (fn) =>
    evaluate(`(() => {
      const c = JSON.parse(sessionStorage.getItem('preview-config') ?? 'null');
      (${fn})(c);
      sessionStorage.setItem('preview-config', JSON.stringify(c));
    })()`);
  // Header labels in order, the sorted column, and whether every first row
  // cell sits under its header.
  const table = () =>
    evaluate(`(() => {
      const ths = [...document.querySelectorAll('thead tr th')];
      const row = document.querySelector('tbody tr');
      const tds = [...row.children];
      return {
        labels: ths.map((th) => th.textContent.trim()),
        sorted: ths.map((th) => th.getAttribute('aria-sort')).join(),
        aligned: ths.length === tds.length && ths.every((th, i) => Math.abs(th.getBoundingClientRect().left - tds[i].getBoundingClientRect().left) < 1 && Math.abs(th.getBoundingClientRect().width - tds[i].getBoundingClientRect().width) < 1),
        cells: tds.map((td) => td.textContent.trim()),
        nameWidth: ths[0].getBoundingClientRect().width,
        widths: Object.fromEntries(ths.map((th) => [th.textContent.trim(), th.getBoundingClientRect().width])),
        actionsClass: ths.at(-1).className,
        fill: Math.round(document.querySelector('.process-table-region').getBoundingClientRect().right - ths.at(-1).getBoundingClientRect().right),
        animations: document.getAnimations().filter((a) => a.id === 'reorder').length,
      };
    })()`);
  const th = (label) =>
    `[...document.querySelectorAll('thead tr th')].find((th) => th.textContent.trim() === ${JSON.stringify(label)})`;
  const cardOrder = (sel) =>
    evaluate(
      `[...document.querySelector(${JSON.stringify(sel)}).children].map((c) => c.querySelector('${sel === ".resource-grid" ? "[id^=resource-trigger-]" : ".card-head"}')?.${sel === ".resource-grid" ? "id.replace('resource-trigger-', '')" : "textContent.trim().split(/\\\\s+/)[0]"} ?? '?')`,
    );

  for (const lang of ["zh", "en"]) {
    const s = lang === "zh" ? zh : en;
    const label = (col) => s[`col.${col}`];
    const at = (what) => `${lang}: ${what}`;
    // Every data column (between the name and actions columns) has one
    // width, within 1 px of rounding.
    const equalData = (t, where) => {
      const w = Object.values(t.widths)
        .slice(1, -1)
        .map((x) => Math.round(x * 10) / 10);
      check(
        Math.max(...w) - Math.min(...w) <= 1,
        at(`${where}: data columns not one width ${w}`),
      );
      return w;
    };
    const widestMin = (t) =>
      Math.max(
        ...Object.keys(t.widths)
          .slice(1, -1)
          .map((l) => columnWidth(idOf(s, l))),
      );
    await viewport(1920, 1000);
    await send("Page.navigate", { url: `${base}/main-page-panel.html` });
    await evaluate("sessionStorage.clear()");
    await open(lang, "processes");

    const grips = () =>
      evaluate(`(() => {
        const ths = [...document.querySelectorAll('thead tr th')];
        const pseudo = (th) => getComputedStyle(th.classList.contains('text-right') && th.querySelector('button') || th, '::before');
        const cards = [...document.querySelectorAll('.resource-grid > *')];
        return {
          headers: ths.map((th) => pseudo(th).content !== 'none'),
          headerHints: ths.slice(1, -1).every((th) => th.querySelector('button').title.endsWith(${JSON.stringify(s["col.moveHint"])})),
          hovered: ths.map((th) => pseudo(th).opacity),
          cards: cards.map((c) => !!c.querySelector('.sc-title > .drag-grip[aria-hidden="true"]')),
          cardHints: cards.every((c) => c.querySelector('.sc-title').title === ${JSON.stringify(s["layout.moveCard"])}),
          cardTitleHints: cards.every((c) => c.querySelector('.sc-title .truncate').title.endsWith(${JSON.stringify(s["layout.moveCard"])})),
          focusable: document.querySelectorAll('.drag-grip[tabindex]').length,
        };
      })()`);
    const g = await grips();
    check(
      g.headers[0] === false &&
        g.headers.at(-1) === false &&
        g.headers.slice(1, -1).every(Boolean) &&
        g.headerHints,
      at(`header grips or hints wrong ${JSON.stringify(g.headers)}`),
    );
    check(
      g.cards.length === 7 &&
        g.cards.every(Boolean) &&
        g.cardHints &&
        g.cardTitleHints &&
        !g.focusable,
      at(`card grips or hints wrong ${JSON.stringify(g)}`),
    );
    const ram = await center(th(label("memory_usage")));
    const memCard = await center(
      "document.querySelector('#resource-trigger-memory').closest('.resource-grid > *')",
    );
    await mouse("mouseMoved", ram.x, ram.y);
    await sleep(200);
    const ramIndex = (await table()).labels.indexOf(label("memory_usage"));
    const hoverHead = (await grips()).hovered;
    check(
      Number(hoverHead[ramIndex]) === 1 && Number(hoverHead[1]) < 1,
      at(`hovered header grip not stronger ${hoverHead}`),
    );
    const clipGrip = async (name) =>
      writeFileSync(
        `${out}/${name}`,
        Buffer.from(
          (
            await send("Page.captureScreenshot", {
              format: "png",
              clip: { x: 0, y: 90, width: 1200, height: 420, scale: 1.5 },
            })
          ).result.data,
          "base64",
        ),
      );
    await clipGrip(`${lang}-1920-grips-header-hover.png`);
    await mouse("mouseMoved", memCard.x, memCard.y);
    await sleep(200);
    check(
      (await evaluate(
        "getComputedStyle(document.querySelector('#resource-trigger-memory').closest('.resource-grid > *').querySelector('.drag-grip')).opacity",
      )) === "1",
      at("hovered card grip not stronger"),
    );
    await clipGrip(`${lang}-1920-grips-card-hover.png`);
    if (lang === "zh") {
      await evaluate("localStorage.setItem('theme', 'githubLight')");
      await open(lang, "processes");
      await mouse("mouseMoved", ram.x, ram.y);
      await sleep(200);
      check(
        (await grips()).headers.slice(1, -1).every(Boolean),
        at("no header grips in the light theme"),
      );
      await clipGrip(`${lang}-1920-grips-light.png`);
      await evaluate("localStorage.removeItem('theme')");
      await open(lang, "processes");
    }

    // Width at full screen.
    let t = await table();
    const long = await evaluate(`(() => {
      const td = [...document.querySelectorAll('tbody tr td:first-child')].find((td) => td.textContent.includes('Very Long'));
      const text = [...td.querySelectorAll('*')].find((e) => e.children.length === 0 && e.textContent.includes('Very Long'));
      return { cut: text.scrollWidth > text.clientWidth, title: !!text.closest('[title]')?.getAttribute('title')?.includes('Very Long') };
    })()`);
    check(
      t.nameWidth <= nameMaxWidth(false) + 1,
      at(
        `name column ${t.nameWidth}px wide at 1920, over ${nameMaxWidth(false)}`,
      ),
    );
    check(
      long.cut && long.title,
      at(`long name not cut with a tooltip ${JSON.stringify(long)}`),
    );
    check(t.aligned, at("header and body columns out of line at 1920"));
    const dataWidths = equalData(t, "1920");
    check(
      t.actionsClass.includes("actions-heading") &&
        Math.abs(t.fill) <= 1 &&
        Math.min(...dataWidths) > widestMin(t),
      at(
        `actions not at the right edge (${t.fill}px) or data columns did not grow ${dataWidths}`,
      ),
    );
    report.push({
      lang,
      step: "wide",
      nameWidth: t.nameWidth,
      dataWidths,
      long,
    });
    await shot(`${lang}-1920-flat.png`);

    // Drag CPU onto the left half of PID: CPU becomes the first column after the name.
    const before = t;
    const cpu = await center(th(label("cpu_usage")));
    const pid = await center(th(label("pid")));
    await drag(cpu, { x: pid.left + 6, y: pid.y }, { hold: true });
    await sleep(250);
    const live = await table();
    const lifted = await evaluate(`(() => {
      const th = ${th(label("cpu_usage"))};
      const cells = [...document.querySelectorAll('tbody td[data-col="cpu_usage"]')];
      return {
        dragging: th.dataset.dragging === '' && cells.every((c) => c.dataset.dragging === ''),
        translate: th.style.translate,
        follows: Math.round(th.getBoundingClientRect().left + th.getBoundingClientRect().width / 2),
        cellsFollow: cells.every((c) => c.style.translate === th.style.translate),
      };
    })()`);
    const midSaved = (await saved()).columnOrder ?? [];
    check(
      live.labels[1] === label("cpu_usage") &&
        live.labels[2] === label("pid") &&
        live.aligned,
      at(
        `neighbours did not move aside while dragging: ${live.labels.slice(0, 4).join(" | ")}`,
      ),
    );
    check(
      lifted.dragging &&
        lifted.cellsFollow &&
        Math.abs(lifted.follows - (pid.left + 6)) < 30,
      at(
        `dragged column not lifted under the pointer ${JSON.stringify(lifted)}`,
      ),
    );
    check(
      !midSaved.length,
      at(`order saved before release ${JSON.stringify(midSaved)}`),
    );
    report.push({
      lang,
      step: "lifted",
      labels: live.labels.slice(0, 4),
      lifted,
    });
    await shot(`${lang}-1920-header-dragging.png`);
    writeFileSync(
      `${out}/${lang}-1920-header-dragging-zoom.png`,
      Buffer.from(
        (
          await send("Page.captureScreenshot", {
            format: "png",
            clip: { x: 0, y: pid.y - 60, width: 900, height: 160, scale: 2 },
          })
        ).result.data,
        "base64",
      ),
    );
    await mouse("mouseReleased", pid.left + 6, pid.y);
    await sleep(200);
    t = await table();
    check(
      t.labels[0] === label("name") &&
        t.labels[1] === label("cpu_usage") &&
        t.labels[2] === label("pid"),
      at(`drag did not move CPU before PID: ${t.labels.join(" | ")}`),
    );
    check(
      t.sorted ===
        before.sorted
          .split(",")
          .map((v, i, a) => a[before.labels.indexOf(t.labels[i])])
          .join(),
      at("the drag changed the sort"),
    );
    check(
      t.aligned && /%/.test(t.cells[1]) && /^\d+$/.test(t.cells[2]),
      at(
        `body cells did not move with the header: ${t.cells.slice(0, 3).join(" | ")}`,
      ),
    );
    let b = await saved();
    check(
      b.columnOrder?.[0] === "cpu_usage" &&
        new Set(b.columnOrder).size === b.columnOrder.length &&
        b.columnOrder.length === 17,
      at(`saved columnOrder ${JSON.stringify(b.columnOrder)}`),
    );
    await shot(`${lang}-1920-header-dropped.png`);

    // Dropping on the name column or past the actions column keeps the name first and actions last.
    const user = await center(th(label("user")));
    const name = await center(th(label("name")));
    await drag(user, { x: name.left + 10, y: name.y });
    t = await table();
    check(
      t.labels[0] === label("name") && t.labels[1] === label("user"),
      at(`drop on name: ${t.labels.slice(0, 3).join(" | ")}`),
    );
    const actions = await center(
      "document.querySelector('thead th.actions-heading')",
    );
    const user2 = await center(th(label("user")));
    await drag(user2, { x: actions.x, y: actions.y });
    t = await table();
    check(
      t.labels.at(-2) === label("user") &&
        t.actionsClass.includes("actions-heading") &&
        t.aligned,
      at(`drop on actions: ${t.labels.slice(-3).join(" | ")}`),
    );

    // Escape mid drag: nothing moves and nothing sorts.
    const snap = await table();
    // Released back over the same button, so the browser does make a click.
    const status = await center(
      `(${th(label("status"))}).querySelector("button")`,
    );
    await drag(status, { x: status.x + 240, y: status.y }, { hold: true });
    await sleep(250);
    const moving = await table();
    check(
      moving.labels.join() !== snap.labels.join(),
      at("dragging Status past a neighbour moved nothing"),
    );
    await press("Escape", "Escape");
    await mouse("mouseMoved", status.x, status.y, 1);
    await mouse("mouseReleased", status.x, status.y);
    await sleep(200);
    t = await table();
    check(
      t.labels.join() === snap.labels.join() && t.sorted === snap.sorted,
      at("Escape changed the order or sorted"),
    );
    check(
      !(await evaluate(
        "!!document.querySelector('[data-dragging]') || [...document.querySelectorAll('th, td')].some((e) => e.style.translate) || 'reordering' in document.documentElement.dataset",
      )),
      at("drag marks left behind after Escape"),
    );

    // A press that moves under the threshold is still a click that sorts.
    const pid2 = await center(`(${th(label("pid"))}).querySelector("button")`);
    await drag(pid2, { x: pid2.x + 3, y: pid2.y }, { steps: 2 });
    t = await table();
    check(
      t.labels.join() === snap.labels.join() &&
        (await evaluate(`(${th(label("pid"))}).getAttribute('aria-sort')`)) !==
          "none",
      at("a short click on PID did not sort"),
    );
    report.push({ lang, step: "header", labels: t.labels, sorted: t.sorted });

    // Alt+Right on the focused header button moves it one place, focus stays.
    await evaluate(`(${th(label("status"))}).querySelector('button').focus()`);
    const i0 = (await table()).labels.indexOf(label("status"));
    await press("ArrowRight", "ArrowRight", 1);
    t = await table();
    check(
      t.labels.indexOf(label("status")) === i0 + 1 &&
        (await evaluate(
          `(${th(label("status"))}).contains(document.activeElement)`,
        )),
      at("Alt+Right did not move the focused header"),
    );
    const order = (await saved()).columnOrder;

    // Hidden columns keep their place; reload keeps the order.
    await editSaved(`(c) => { c.appearance.columnVisibility.pid = false; }`);
    await open(lang, "processes");
    t = await table();
    check(
      !t.labels.includes(label("pid")) &&
        t.aligned &&
        t.labels[1] === label("cpu_usage"),
      at(`hidden PID: ${t.labels.join(" | ")}`),
    );
    report.push({
      lang,
      step: "pid-hidden",
      dataWidths: equalData(t, "PID hidden"),
    });
    await shot(`${lang}-1920-pid-hidden.png`);
    await editSaved(
      `(c) => { c.appearance.columnVisibility.pid = true; c.behavior.processView = 'tree'; }`,
    );
    await open(lang, "processes");
    t = await table();
    // Open every family so the deepest and the long name show.
    for (let i = 0; i < 6; i++) {
      const more = await evaluate(
        `(() => { const b = document.querySelector('tbody [aria-expanded="false"]'); b?.click(); return !!b; })()`,
      );
      if (!more) break;
      await sleep(200);
    }
    t = await table();
    const tree = await evaluate(`(() => {
      const cells = [...document.querySelectorAll('tbody tr td:first-child')];
      const text = (td) => [...td.querySelectorAll('*')].find((e) => e.children.length === 0 && e.textContent.trim().length > 3);
      const long = cells.find((td) => td.textContent.includes('Very Long'));
      const deep = cells.find((td) => td.textContent.includes('Renderer'));
      return {
        rows: cells.length,
        longCut: !!long && text(long).scrollWidth > text(long).clientWidth,
        deepInside: !!deep && text(deep).getBoundingClientRect().right <= deep.getBoundingClientRect().right + 0.5,
        indent: !!deep && text(deep).getBoundingClientRect().left - cells[0].getBoundingClientRect().left,
      };
    })()`);
    check(
      tree.rows === 7 && tree.longCut && tree.deepInside && tree.indent > 30,
      at(`tree rows ${JSON.stringify(tree)}`),
    );
    check(
      JSON.stringify((await saved()).columnOrder) === JSON.stringify(order),
      at("reload changed the saved column order"),
    );
    check(
      t.labels[0] === label("name") &&
        t.labels[2] === label("pid") &&
        t.aligned,
      at(`tree view order: ${t.labels.join(" | ")}`),
    );
    check(
      t.nameWidth <= nameMaxWidth(true) + 1,
      at(`tree name column ${t.nameWidth}px`),
    );
    report.push({
      lang,
      step: "tree",
      labels: t.labels,
      nameWidth: t.nameWidth,
      rows: tree,
    });
    report.push({
      lang,
      step: "tree-widths",
      dataWidths: equalData(t, "tree"),
    });
    await shot(`${lang}-1920-tree.png`);
    await editSaved(`(c) => { c.behavior.processView = 'flat'; }`);
    await open(lang, "processes");

    // Process cards: drag CPU's title after Network, nothing opens, then a click does.
    const cards0 = await cardOrder(".resource-grid");
    const cpuTitle = await center(
      `document.querySelector('#resource-trigger-cpu').closest('.resource-grid > *').querySelector('.sc-title')`,
    );
    const net = await center(
      `document.querySelector('#resource-trigger-network').closest('.resource-grid > *')`,
    );
    await drag(cpuTitle, { x: net.right - 8, y: net.y }, { hold: true });
    await sleep(250);
    const cardsLive = await cardOrder(".resource-grid");
    check(
      cardsLive.indexOf("cpu") === cardsLive.indexOf("network") + 1 &&
        (await evaluate(
          "document.querySelector('#resource-trigger-cpu').closest('.resource-grid > *').dataset.dragging === ''",
        )) &&
        !((await saved()).processCards ?? []).length,
      at(`process cards did not make room while dragging: ${cardsLive.join()}`),
    );
    await shot(`${lang}-1920-card-dragging.png`);
    await mouse("mouseReleased", net.right - 8, net.y);
    await sleep(450);
    let cards = await cardOrder(".resource-grid");
    check(
      (await evaluate(
        "document.getAnimations().filter((a) => a.id === 'reorder').length",
      )) === 0,
      at("reorder animations still running after the drop"),
    );
    const sheet = "!!document.querySelector('[data-slot=\"sheet-content\"]')";
    check(
      cards.indexOf("cpu") === cards.indexOf("network") + 1 &&
        cards.length === 7,
      at(`process cards ${cards0.join()} -> ${cards.join()}`),
    );
    check(
      !(await evaluate(sheet)),
      at("dragging a process card opened its details"),
    );
    await evaluate(
      "document.querySelector('#resource-trigger-memory').scrollIntoView({block:'center'})",
    );
    const mem = await center(
      "document.querySelector('#resource-trigger-memory')",
    );
    await mouse("mouseMoved", mem.x, mem.y);
    await mouse("mousePressed", mem.x, mem.y, 1);
    await mouse("mouseReleased", mem.x, mem.y);
    check(
      await waitFor(sheet, 3000),
      at("memory details did not open after a card drag"),
    );
    await shot(`${lang}-1920-card-click-after-drag.png`);
    await press("Escape", "Escape");
    await sleep(400);

    // Dashboard cards: drag CPU's head after Memory, nothing opens, then a click does.
    await evaluate("document.querySelectorAll('nav.tabs button')[0].click()");
    await waitFor("document.querySelectorAll('.card-head').length >= 4");
    await sleep(300);
    const grid =
      "document.querySelector('.card-head').closest('.card').parentElement";
    const dash0 = await evaluate(
      `[...${grid}.children].map((c) => c.querySelector('.card-head')?.textContent.trim())`,
    );
    // Started on the grip itself, which sits in the card's padding.
    const head = await center(
      `${grid}.children[0].querySelector('.card-head > .drag-grip')`,
    );
    check(
      (await evaluate(
        `document.querySelectorAll('.card-head > .drag-grip[aria-hidden="true"]').length`,
      )) === dash0.length &&
        (await evaluate(
          `${grid}.children[0].querySelector('.card-head').title`,
        )) === s["layout.moveCard"],
      at("dashboard card grips or hints missing"),
    );
    const third = await center(`${grid}.children[2]`);
    await drag(head, { x: third.right - 8, y: third.y }, { hold: true });
    await sleep(250);
    const dashLive = await evaluate(
      `[...${grid}.children].map((c) => c.querySelector('.card-head')?.textContent.trim())`,
    );
    check(
      dashLive[2] === dash0[0] &&
        (await evaluate(`${grid}.children[2].dataset.dragging === ''`)) &&
        !((await saved()).dashboardCards ?? []).length,
      at(
        `dashboard cards did not make room while dragging: ${dashLive.join(" / ")}`,
      ),
    );
    await shot(`${lang}-1920-dashboard-dragging.png`);
    await mouse("mouseReleased", third.right - 8, third.y);
    await sleep(250);
    const dash = await evaluate(
      `[...${grid}.children].map((c) => c.querySelector('.card-head')?.textContent.trim())`,
    );
    check(
      dash[2] === dash0[0] && dash.length === dash0.length,
      at(`dashboard ${dash0.join(" / ")} -> ${dash.join(" / ")}`),
    );
    check(
      await evaluate("document.querySelectorAll('.card-head').length >= 4"),
      at("dragging a dashboard card opened its detail"),
    );
    const savedDash = (await saved()).dashboardCards;
    const card2 = await center(`${grid}.children[0]`);
    await mouse("mouseMoved", card2.x, card2.y + 30);
    await mouse("mousePressed", card2.x, card2.y + 30, 1);
    await mouse("mouseReleased", card2.x, card2.y + 30);
    check(
      await waitFor("document.querySelectorAll('.card-head').length < 4", 3000),
      at("a dashboard card click after a drag opened nothing"),
    );
    await shot(`${lang}-1920-dashboard-click-after-drag.png`);

    // Reload keeps both card orders.
    await open(lang, "dashboard");
    check(
      JSON.stringify(
        await evaluate(
          `[...${grid}.children].map((c) => c.querySelector('.card-head')?.textContent.trim())`,
        ),
      ) === JSON.stringify(dash),
      at("dashboard order lost on reload"),
    );
    check(
      JSON.stringify((await saved()).dashboardCards) ===
        JSON.stringify(savedDash),
      at("saved dashboard order changed"),
    );
    await open(lang, "processes");
    check(
      JSON.stringify(await cardOrder(".resource-grid")) ===
        JSON.stringify(cards),
      at("process card order lost on reload"),
    );
    report.push({ lang, step: "cards", process: cards, dashboard: dash });

    // Bad saved orders still show everything once.
    await editSaved(
      `(c) => { c.behavior.columnOrder = ['pid', 'pid', 'bogus']; c.behavior.processCards = ['network', 'cpu', 'cpu', 'x']; c.behavior.dashboardCards = ['temp', 'temp', 'nope']; }`,
    );
    await open(lang, "processes");
    t = await table();
    cards = await cardOrder(".resource-grid");
    check(
      new Set(t.labels).size === t.labels.length &&
        t.labels[1] === label("pid") &&
        t.aligned,
      at(`bad column order: ${t.labels.join(" | ")}`),
    );
    check(
      cards.length === 7 &&
        new Set(cards).size === 7 &&
        cards.slice(0, 3).join() === "network,temp,cpu",
      at(`bad process cards: ${cards.join()}`),
    );
    await evaluate("document.querySelectorAll('nav.tabs button')[0].click()");
    await sleep(400);
    const dashBad = await evaluate(
      `[...${grid}.children].map((c) => c.querySelector('.card-head')?.textContent.trim())`,
    );
    check(
      new Set(dashBad).size === dashBad.length &&
        dashBad.length === dash.length,
      at(`bad dashboard cards: ${dashBad.join(" / ")}`),
    );

    // Narrow window: name at its narrowest, the table scrolls sideways, cards wrap.
    await viewport(1024, 760);
    await open(lang, "processes");
    t = await table();
    const narrow = await evaluate(
      `(() => { const r = document.querySelector('thead').closest('[data-slot="table-container"], .overflow-auto, .overflow-x-auto') ?? document.querySelector('table').parentElement; const tops = new Set([...document.querySelector('.resource-grid').children].map((c) => Math.round(c.getBoundingClientRect().top))); return { scrolls: r.scrollWidth > r.clientWidth, rows: tops.size }; })()`,
    );
    check(
      Math.abs(t.nameWidth - nameMinWidth(false)) <= 1 &&
        narrow.scrolls &&
        t.aligned,
      at(`1024: name ${t.nameWidth}px, ${JSON.stringify(narrow)}`),
    );
    report.push({ lang, step: "narrow", nameWidth: t.nameWidth, ...narrow });
    const narrowData = equalData(t, "1024");
    check(
      Math.abs(narrowData[0] - widestMin(t)) <= 1,
      at(
        `1024: data columns ${narrowData[0]}px, not their shared minimum ${widestMin(t)}`,
      ),
    );
    report.push({ lang, step: "narrow-widths", dataWidths: narrowData });
    await shot(`${lang}-1024-flat.png`);
    await evaluate("document.querySelectorAll('nav.tabs button')[0].click()");
    await sleep(400);
    await shot(`${lang}-1024-dashboard.png`);
  }

  await send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-motion", value: "reduce" }],
  });
  await viewport(1920, 1000);
  await open("en", "processes");
  const rmFrom = await center(
    `[...document.querySelectorAll('thead tr th')][3]`,
  );
  const rmTo = await center(`[...document.querySelectorAll('thead tr th')][1]`);
  await drag(rmFrom, { x: rmTo.left + 4, y: rmTo.y }, { hold: true });
  const rm = await evaluate(
    "({ animations: document.getAnimations().filter((a) => a.id === 'reorder').length, lifted: !!document.querySelector('th[data-dragging]') })",
  );
  await press("Escape", "Escape");
  await mouse("mouseReleased", rmTo.left + 4, rmTo.y);
  check(
    rm.lifted && rm.animations === 0,
    `reduced motion: neighbours still animate ${JSON.stringify(rm)}`,
  );
  report.push({ step: "reduced-motion", ...rm });
  ws.close();
} catch (error) {
  problems.push(`run failed: ${error.stack ?? error}`);
} finally {
  chrome.kill();
  await server.close();
}
writeFileSync(
  `${out}/ui-reorder.json`,
  JSON.stringify({ problems, report }, null, 2),
);
console.log(JSON.stringify(report, null, 1));
if (problems.length) {
  console.error(`FAIL\n${problems.join("\n")}`);
  process.exit(1);
}
console.log(`PASS ui-reorder -> ${out}`);
