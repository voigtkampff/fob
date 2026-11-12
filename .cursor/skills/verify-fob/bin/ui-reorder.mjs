// ui-reorder.mjs <evidence-dir>: the real main window route (+page.svelte,
// production CSS) with generic mocked processes and readings. With real
// pointer and key events, in zh and en, it checks:
// - process column headers drag into a new order, header and body cells move
//   together, the name column stays first, the actions column takes no drop,
//   a short click still sorts, Escape cancels without sorting, Alt+Arrow
//   on a focused header or card moves and saves nothing, hidden columns and
//   tree view keep their place;
// - while a header or card is dragged it lifts and follows the pointer, the
//   others already stand in the order it would land in (header and body
//   cells together), and nothing is saved until release; with reduced
//   motion they jump instead of sliding;
// - every summary card shows a dot grip whose left edge lines up with the
//   value below, stronger on hover, and a Dashboard card drags from its
//   grip; column headers show no grip and no hover explanation (including
//   sort, resize and drag hints); no vertical lines part the columns;
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
import { columnWidth } from "../../../../src/lib/utils/table-layout.js";

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
      const ths = [...document.querySelectorAll('thead tr th:not(.col-filler)')];
      const row = document.querySelector('tbody tr');
      const tds = [...row.children].filter((td) => !td.classList.contains('col-filler'));
      return {
        labels: ths.map((th) => th.textContent.trim()),
        sorted: ths.map((th) => th.getAttribute('aria-sort')).join(),
        aligned: ths.length === tds.length && ths.every((th, i) => Math.abs(th.getBoundingClientRect().left - tds[i].getBoundingClientRect().left) < 1 && Math.abs(th.getBoundingClientRect().width - tds[i].getBoundingClientRect().width) < 1),
        cells: tds.map((td) => td.textContent.trim()),
        nameWidth: ths[0].getBoundingClientRect().width,
        widths: Object.fromEntries(ths.map((th) => [th.textContent.trim(), th.getBoundingClientRect().width])),
        actionsClass: ths.at(-1).className,
        filler: document.querySelector('thead th.col-filler')?.getBoundingClientRect().width ?? -1,
        fill: Math.round(document.querySelector('.process-table-region').getBoundingClientRect().right - ths.at(-1).getBoundingClientRect().right),
        animations: document.getAnimations().filter((a) => a.id === 'reorder').length,
      };
    })()`);
  const th = (label) =>
    `[...document.querySelectorAll('thead tr th:not(.col-filler)')].find((th) => th.textContent.trim() === ${JSON.stringify(label)})`;
  const cardOrder = (sel) =>
    evaluate(
      `[...document.querySelector(${JSON.stringify(sel)}).children].map((c) => c.querySelector('${sel === ".resource-grid" ? "[id^=resource-trigger-]" : ".card-head"}')?.${sel === ".resource-grid" ? "id.replace('resource-trigger-', '')" : "textContent.trim().split(/\\\\s+/)[0]"} ?? '?')`,
    );

  for (const lang of ["zh", "en"]) {
    const s = lang === "zh" ? zh : en;
    const label = (col) => s[`col.${col}`];
    const at = (what) => `${lang}: ${what}`;
    // Every data column keeps its own content width (or the one set by
    // hand), within 1 px of rounding.
    const ownWidths = (t, where, tree = false, manual = {}) => {
      const w = Object.fromEntries(
        Object.entries(t.widths)
          .slice(1, -1)
          .map(([l, x]) => [idOf(s, l), Math.round(x * 10) / 10]),
      );
      const off = Object.entries(w).filter(
        ([id, x]) => Math.abs(x - (manual[id] ?? columnWidth(id, tree))) > 1,
      );
      check(
        off.length === 0,
        at(`${where}: columns off their width ${JSON.stringify(off)}`),
      );
      return w;
    };
    await viewport(1920, 1000);
    await send("Page.navigate", { url: `${base}/main-page-panel.html` });
    await evaluate("sessionStorage.clear()");
    await open(lang, "processes");

    const grips = () =>
      evaluate(`(() => {
        const ths = [...document.querySelectorAll('thead tr th:not(.col-filler)')];
        const tds = [...document.querySelector('tbody tr').children].filter((td) => !td.classList.contains('col-filler'));
        const pseudo = (el) => getComputedStyle(el, '::before').content;
        const cards = [...document.querySelectorAll('.resource-grid > *')];
        const rule = (el) => getComputedStyle(el).boxShadow;
        return {
          headerGrips: ths.filter((th) => pseudo(th) !== 'none' || (th.querySelector('button') && pseudo(th.querySelector('button')) !== 'none')).length,
          headerHints: ths.slice(0, -1).every((th) => {
            const btn = th.querySelector('button');
            const resize = th.querySelector('.col-resize');
            return (!btn || !btn.title) && (!resize || !resize.title);
          }),
          cards: cards.map((c) => !!c.querySelector('.sc-title > .drag-grip[aria-hidden="true"]')),
          gripOffsets: cards.map((c) => Math.round((c.querySelector('.drag-grip').getBoundingClientRect().left - c.querySelector('.sc-value').getBoundingClientRect().left) * 10) / 10),
          cardHints: cards.every((c) => !c.querySelector('.sc-title').title),
          cardTitleHints: cards.every((c) => {
            const tip = c.querySelector('.sc-title .truncate')?.title ?? '';
            return tip.length > 0 && !/Drag to reorder|拖动调整顺序/.test(tip);
          }),
          focusable: document.querySelectorAll('.drag-grip[tabindex]').length,
          firstRules: [rule(ths[0]), rule(tds[0])],
          headRules: ths.slice(1, -1).map(rule),
          cellRules: tds.slice(1, -1).map(rule),
        };
      })()`);
    // No vertical line between columns: header cells keep only their
    // bottom edge and body cells draw none.
    const ruled = (g, where) => {
      const line = /1px 0px 0px 0px inset/;
      const lined = [...g.firstRules, ...g.headRules, ...g.cellRules].filter(
        (r) => line.test(r),
      );
      check(
        !lined.length,
        at(`${where}: vertical column lines drawn ${JSON.stringify(lined[0])}`),
      );
      return lined.length > 0;
    };
    const g = await grips();
    check(
      g.headerGrips === 0 && g.headerHints,
      at(`header grips shown or header tooltip still present ${JSON.stringify(g)}`),
    );
    check(
      g.cards.length === 7 &&
        g.cards.every(Boolean) &&
        g.gripOffsets.every((d) => Math.abs(d) <= 1) &&
        g.cardHints &&
        g.cardTitleHints &&
        !g.focusable,
      at(`card grips, alignment or leftover drag hint ${JSON.stringify(g)}`),
    );
    report.push({
      lang,
      step: "grips",
      cardGripMinusValueLeft: g.gripOffsets,
      columnLines: ruled(g, "dark"),
    });
    // Owners show account names in full; an owner with no account keeps its
    // SID, cut in the cell and whole in the tooltip.
    const owners = await evaluate(
      `[...document.querySelectorAll('tbody td[data-col="user"] > div')].map((d) => ({ text: d.textContent.trim(), title: d.title, fits: d.scrollWidth <= d.clientWidth, mono: getComputedStyle(d.firstElementChild ?? d).fontFamily.includes('mono') }))`,
    );
    const sid = "S-1-5-21-1000000001-1000000002-1000000003-1999";
    check(
      ["SYSTEM", "LOCAL SERVICE", "sample-user"].every((n) =>
        owners.some((o) => o.text === n && o.fits && !o.mono),
      ) && owners.some((o) => o.text === sid && o.title === sid),
      at(`owner names wrong ${JSON.stringify(owners)}`),
    );
    const memCard = await center(
      "document.querySelector('#resource-trigger-memory').closest('.resource-grid > *')",
    );
    const clip = async (name, rect) =>
      writeFileSync(
        `${out}/${name}`,
        Buffer.from(
          (
            await send("Page.captureScreenshot", {
              format: "png",
              clip: { ...rect, scale: 2 },
            })
          ).result.data,
          "base64",
        ),
      );
    const cardsClip = { x: 0, y: 90, width: 560, height: 200 };
    const headClip = { x: 0, y: 410, width: 1100, height: 140 };
    await mouse("mouseMoved", memCard.x, memCard.y);
    await sleep(200);
    check(
      (await evaluate(
        "getComputedStyle(document.querySelector('#resource-trigger-memory').closest('.resource-grid > *').querySelector('.drag-grip')).opacity",
      )) === "1",
      at("hovered card grip not stronger"),
    );
    await clip(`${lang}-1920-card-grips.png`, cardsClip);
    await mouse("mouseMoved", 5, 5);
    await clip(`${lang}-1920-table-header.png`, headClip);
    if (lang === "zh") {
      await evaluate("localStorage.setItem('theme', 'githubLight')");
      await open(lang, "processes");
      const light = await grips();
      report.push({
        lang,
        step: "grips-light",
        columnLines: ruled(light, "light"),
      });
      check(
        light.gripOffsets.every((d) => Math.abs(d) <= 1),
        at(`light: card grips not over the values ${light.gripOffsets}`),
      );
      await clip(`${lang}-1920-card-grips-light.png`, cardsClip);
      await clip(`${lang}-1920-table-header-light.png`, headClip);
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
      Math.abs(t.nameWidth - 460) <= 1,
      at(`name column ${t.nameWidth}px wide at 1920, not 460`),
    );
    check(
      long.cut && long.title,
      at(`long name not cut with a tooltip ${JSON.stringify(long)}`),
    );
    check(t.aligned, at("header and body columns out of line at 1920"));
    const dataWidths = ownWidths(t, "1920");
    check(
      t.actionsClass.includes("actions-heading") &&
        Math.abs(t.fill) <= 1 &&
        t.filler > 100,
      at(
        `actions not at the right edge (${t.fill}px) or no empty room before them (${t.filler}px)`,
      ),
    );
    // Every header label fits its column (Chinese and English).
    const cutHeads = await evaluate(
      `[...document.querySelectorAll('thead th button')].filter((b) => b.scrollWidth > b.clientWidth + 0.5).map((b) => b.textContent.trim())`,
    );
    check(
      cutHeads.length === 0,
      at(`cut header labels ${cutHeads.join(" | ")}`),
    );
    report.push({
      lang,
      step: "wide",
      nameWidth: t.nameWidth,
      dataWidths,
      filler: t.filler,
      long,
    });
    await shot(`${lang}-1920-flat.png`);

    // Drag CPU onto the left half of PID: CPU becomes the first column after the name.
    const before = t;
    // A drag starts on the header's title (its sort button), not the cell.
    const cpuTh = await center(th(label("cpu_usage")));
    const cpu = await center(
      `(${th(label("cpu_usage"))}).querySelector("button")`,
    );
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
        Math.abs(lifted.follows - (pid.left + 6 - (cpu.x - cpuTh.x))) < 30,
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
    const user = await center(`(${th(label("user"))}).querySelector("button")`);
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
    const user2 = await center(
      `(${th(label("user"))}).querySelector("button")`,
    );
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

    // Reordering has no shortcut: Alt+Right on a focused header or card
    // moves and saves nothing, and focus stays.
    const savedBefore = JSON.stringify(await saved());
    await evaluate(`(${th(label("status"))}).querySelector('button').focus()`);
    const i0 = (await table()).labels.indexOf(label("status"));
    await press("ArrowRight", "ArrowRight", 1);
    t = await table();
    const cardsBefore = await cardOrder(".resource-grid");
    await evaluate("document.querySelector('#resource-trigger-cpu').focus()");
    await press("ArrowRight", "ArrowRight", 1);
    check(
      t.labels.indexOf(label("status")) === i0 &&
        JSON.stringify(await cardOrder(".resource-grid")) ===
          JSON.stringify(cardsBefore) &&
        JSON.stringify(await saved()) === savedBefore &&
        (await evaluate(
          "document.activeElement === document.querySelector('#resource-trigger-cpu')",
        )),
      at("Alt+Right moved or saved a column or card"),
    );
    // Only a title starts a drag: a press on a value, outside a header's
    // title, or on a card's body or details button lifts nothing.
    const lifts = async (expr, point = "center") => {
      const p = await evaluate(
        `(() => { const e = ${expr}; if (!e) return null; e.scrollIntoView({ block: 'nearest', inline: 'nearest' }); const r = e.getBoundingClientRect(); return ${point === "start" ? "{ x: r.left + 3, y: r.top + r.height / 2 }" : "{ x: r.left + r.width / 2, y: r.top + Math.min(r.height / 2, 14) }"}; })()`,
      );
      if (!p) return `missing ${expr}`;
      await drag(p, { x: p.x + 160, y: p.y + 40 }, { hold: true });
      const lifted = await evaluate(
        "!!document.querySelector('[data-dragging], [data-drag-slot]')",
      );
      await mouse("mouseReleased", p.x + 160, p.y + 40);
      await sleep(200);
      if (
        await evaluate(
          "!!document.querySelector('[data-slot=\"sheet-content\"]')",
        )
      ) {
        await press("Escape", "Escape");
        await sleep(300);
      }
      return lifted;
    };
    const labelsBefore = (await table()).labels.join();
    const cardsAt = JSON.stringify(await cardOrder(".resource-grid"));
    const cpuCard =
      "document.querySelector('#resource-trigger-cpu').closest('.resource-grid > *')";
    const off = {
      cell: await lifts(
        `document.querySelector('tbody td[data-col="cpu_usage"]')`,
      ),
      headPadding: await lifts(`${th(label("memory_usage"))}`, "start"),
      cardValue: await lifts(`${cpuCard}.querySelector('.sc-value')`),
      cardBody: await lifts(`${cpuCard}.querySelector('.sc-middle')`),
      cardButton: await lifts(`${cpuCard}.querySelector('.sc-head button')`),
    };
    check(
      Object.values(off).every((v) => v === false) &&
        (await table()).labels.join() === labelsBefore &&
        JSON.stringify(await cardOrder(".resource-grid")) === cardsAt &&
        JSON.stringify(await saved()) === savedBefore,
      at(`a press off a title started a drag ${JSON.stringify(off)}`),
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
      dataWidths: ownWidths(t, "PID hidden"),
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
      Math.abs(t.nameWidth - 560) <= 1,
      at(`tree name column ${t.nameWidth}px, not 560`),
    );
    const cutTree = await evaluate(
      `[...document.querySelectorAll('thead th button')].filter((b) => b.scrollWidth > b.clientWidth + 0.5).map((b) => b.textContent.trim())`,
    );
    check(
      cutTree.length === 0,
      at(`cut tree header labels ${cutTree.join(" | ")}`),
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
      dataWidths: ownWidths(t, "tree", true),
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
          "document.querySelector('#resource-trigger-cpu').closest('.resource-grid > *').dataset.dragSlot === '' && document.querySelector(':popover-open > [data-dragging]')?.textContent === document.querySelector('#resource-trigger-cpu').closest('.resource-grid > *').textContent",
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
    // Started on the grip itself, which lines up with the value below.
    const head = await center(
      `${grid}.children[0].querySelector('.card-head .drag-grip')`,
    );
    check(
      (await evaluate(
        `document.querySelectorAll('.card-head .drag-grip[aria-hidden="true"]').length`,
      )) === dash0.length &&
        !(await evaluate(
          `${grid}.children[0].querySelector('.card-head').title`,
        )),
      at("dashboard card grips missing or drag hint still present"),
    );
    const dashOffsets = await evaluate(
      `[...${grid}.children].map((c) => Math.round((c.querySelector('.drag-grip').getBoundingClientRect().left - c.children[1].getBoundingClientRect().left) * 10) / 10)`,
    );
    check(
      dashOffsets.every((d) => Math.abs(d) <= 1),
      at(`dashboard grips not over the content below ${dashOffsets}`),
    );
    report.push({
      lang,
      step: "dashboard-grips",
      gripMinusContentLeft: dashOffsets,
    });
    await clip(
      `${lang}-1920-dashboard-grips.png`,
      await evaluate(
        `(() => { const r = ${grid}.getBoundingClientRect(); return { x: r.left - 8, y: r.top - 8, width: Math.min(r.width + 16, 1000), height: 360 }; })()`,
      ),
    );
    const third = await center(`${grid}.children[2]`);
    await drag(head, { x: third.right - 8, y: third.y }, { hold: true });
    await sleep(250);
    const dashLive = await evaluate(
      `[...${grid}.children].map((c) => c.querySelector('.card-head')?.textContent.trim())`,
    );
    check(
      dashLive[2] === dash0[0] &&
        (await evaluate(
          `${grid}.children[2].dataset.dragSlot === '' && !!document.querySelector(':popover-open > [data-dragging]')`,
        )) &&
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

    {
      // Column widths by hand: drag a column's right edge.
      await viewport(1920, 1000);
      await open(lang, "processes");
      const orderBefore = (await saved()).columnOrder ?? [];
      const statusId = "status";
      const own = columnWidth(statusId);
      const edge = (l) =>
        evaluate(
          `(() => { const th = ${th(l)}; const e = th?.querySelector('.col-resize'); if (!e) return null; const r = e.getBoundingClientRect(); const cue = getComputedStyle(e, '::after'); return { x: r.x + r.width / 2, y: r.y + r.height / 2, w: r.width, right: th.getBoundingClientRect().right, cue: cue.opacity === '0' || cue.backgroundColor === 'rgba(0, 0, 0, 0)' ? 'hidden' : 'shown', active: e.hasAttribute('data-active'), cursor: getComputedStyle(e).cursor }; })()`,
        );
      const statusLabel = label(statusId);
      const widthOf = async (l) => (await table()).widths[l];
      let e0 = await edge(statusLabel);
      check(
        !!e0 &&
          e0.w >= 6 &&
          e0.w <= 10 &&
          Math.abs(e0.x + e0.w / 2 - e0.right) <= 1 &&
          e0.cue === "hidden" &&
          e0.cursor === "col-resize",
        at(`resize edge ${JSON.stringify(e0)}`),
      );
      await mouse("mouseMoved", e0.x, e0.y);
      await sleep(150);
      const hovered = await edge(statusLabel);
      check(
        hovered.cue === "shown",
        at(`no cue on hovering the edge ${JSON.stringify(hovered)}`),
      );
      await shot(`${lang}-1920-resize-hover.png`);
      await mouse("mouseMoved", 5, 5);
      await sleep(150);
      check(
        (await edge(statusLabel)).cue === "hidden",
        at("edge cue stays after leaving"),
      );
      const unsized = await table();
      await evaluate(
        `(() => { window.__saves = 0; const set = sessionStorage.setItem.bind(sessionStorage); sessionStorage.setItem = (k, v) => { if (k === 'preview-config') window.__saves++; set(k, v); }; })()`,
      );
      // Esc cancels.
      await drag(e0, { x: e0.x + 80, y: e0.y }, { hold: true });
      let live = await table();
      const during = await edge(statusLabel);
      check(
        Math.abs(live.widths[statusLabel] - (own + 80)) <= 1 &&
          live.aligned &&
          during.active &&
          during.cue === "shown",
        at(`live resize ${live.widths[statusLabel]} ${JSON.stringify(during)}`),
      );
      await shot(`${lang}-1920-resize-drag.png`);
      await press("Escape", "Escape");
      await mouse("mouseReleased", e0.x + 80, e0.y);
      await sleep(150);
      live = await table();
      check(
        Math.abs(live.widths[statusLabel] - own) <= 1 &&
          (await evaluate("window.__saves")) === 0 &&
          !(await saved()).columnWidths?.status,
        at(`Esc did not cancel the resize ${live.widths[statusLabel]}`),
      );
      // Drag and release: one save, only this column changes, no sort or reorder.
      await drag(e0, { x: e0.x + 80, y: e0.y }, { hold: true });
      check(
        (await evaluate("window.__saves")) === 0,
        at("saved unsized the edge was released"),
      );
      await mouse("mouseReleased", e0.x + 80, e0.y);
      await sleep(250);
      const after = await table();
      const savedW = (await saved()).columnWidths ?? {};
      const others = Object.keys(unsized.widths).filter(
        (l) => l !== statusLabel && l !== label("name"),
      );
      check(
        (await evaluate("window.__saves")) === 1 &&
          savedW.status === own + 80 &&
          Math.abs(after.widths[statusLabel] - (own + 80)) <= 1 &&
          others.every(
            (l) => Math.abs(after.widths[l] - unsized.widths[l]) <= 1,
          ) &&
          after.labels.join() === unsized.labels.join() &&
          after.sorted === unsized.sorted &&
          after.cells.join() === unsized.cells.join() &&
          after.aligned,
        at(
          `resize: saves ${await evaluate("window.__saves")}, ${JSON.stringify(savedW)}, labels ${after.labels.join("|")}, sort ${after.sorted}`,
        ),
      );
      report.push({
        lang,
        step: "resize",
        own,
        saved: savedW,
        filler: after.filler,
      });
      // Kept after reload, a reorder, hide and show, tree view, and a narrow window.
      await open(lang, "processes");
      check(
        Math.abs((await widthOf(statusLabel)) - (own + 80)) <= 1,
        at("hand width lost on reload"),
      );
      const pidHead = await center(
        `${th(label("pid"))}.querySelector('button')`,
      );
      const userHead = await center(th(label("user")));
      await drag(pidHead, { x: userHead.x, y: pidHead.y }, { steps: 10 });
      await sleep(400);
      let moved = await table();
      check(
        moved.labels.indexOf(label("pid")) >
          unsized.labels.indexOf(label("pid")) &&
          Math.abs(moved.widths[statusLabel] - (own + 80)) <= 1 &&
          Math.abs(moved.widths[label("pid")] - columnWidth("pid")) <= 1,
        at(
          `hand width after a reorder ${moved.labels.join("|")} ${moved.widths[statusLabel]}`,
        ),
      );
      await editSaved(
        `(c) => { c.appearance.columnVisibility.status = false; }`,
      );
      await open(lang, "processes");
      await editSaved(
        `(c) => { c.appearance.columnVisibility.status = true; c.behavior.processView = 'tree'; }`,
      );
      await open(lang, "processes");
      check(
        Math.abs((await widthOf(statusLabel)) - (own + 80)) <= 1,
        at("hand width lost after hide, show and tree view"),
      );
      await editSaved(`(c) => { c.behavior.processView = 'flat'; }`);
      await viewport(1024, 760);
      await open(lang, "processes");
      check(
        Math.abs((await widthOf(statusLabel)) - (own + 80)) <= 1,
        at("hand width lost in a narrow window"),
      );
      await viewport(1920, 1000);
      await open(lang, "processes");
      // Double-click the edge: back to the automatic width, the saved key gone.
      e0 = await edge(statusLabel);
      for (const clickCount of [1, 2]) {
        await send("Input.dispatchMouseEvent", {
          type: "mousePressed",
          x: e0.x,
          y: e0.y,
          button: "left",
          buttons: 1,
          clickCount,
        });
        await send("Input.dispatchMouseEvent", {
          type: "mouseReleased",
          x: e0.x,
          y: e0.y,
          button: "left",
          buttons: 0,
          clickCount,
        });
      }
      await sleep(250);
      const reset = await table();
      check(
        Math.abs(reset.widths[statusLabel] - own) <= 1 &&
          !("status" in ((await saved()).columnWidths ?? {})) &&
          reset.sorted === moved.sorted,
        at(
          `double-click did not reset ${reset.widths[statusLabel]} ${JSON.stringify((await saved()).columnWidths)}`,
        ),
      );
      // Put the column order back for the checks below.
      await editSaved(
        `(c) => { c.behavior.columnOrder = ${JSON.stringify(orderBefore)}; }`,
      );
    }

    // Narrow window: name at its narrowest, the table scrolls sideways, cards wrap.
    await viewport(1024, 760);
    await open(lang, "processes");
    t = await table();
    const narrow = await evaluate(
      `(() => { const r = document.querySelector('thead').closest('[data-slot="table-container"], .overflow-auto, .overflow-x-auto') ?? document.querySelector('table').parentElement; const tops = new Set([...document.querySelector('.resource-grid').children].map((c) => Math.round(c.getBoundingClientRect().top))); return { scrolls: r.scrollWidth > r.clientWidth, rows: tops.size }; })()`,
    );
    check(
      Math.abs(t.nameWidth - 180) <= 1 &&
        t.filler <= 0.5 &&
        narrow.scrolls &&
        t.aligned,
      at(`1024: name ${t.nameWidth}px, ${JSON.stringify(narrow)}`),
    );
    report.push({ lang, step: "narrow", nameWidth: t.nameWidth, ...narrow });
    const narrowData = ownWidths(t, "1024");
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
    `[...document.querySelectorAll('thead tr th:not(.col-filler)')][3].querySelector('button')`,
  );
  const rmTo = await center(
    `[...document.querySelectorAll('thead tr th:not(.col-filler)')][1]`,
  );
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
