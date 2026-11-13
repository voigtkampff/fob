// Original SettingsWindow/SensorSetup with the existing mocked Tauri fixture.
// helper-settings.mjs <zh|en> <evidence-dir> [reproduce]
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
const [lang = "en", out = "/workspace/scratch/verify/evidence", mode] =
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
const port = lang === "zh" ? 9351 : 9352;
const profile = mkdtempSync(join(tmpdir(), "fob-helper-"));
const chrome = spawn(
  "google-chrome",
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    `--user-data-dir=${profile}`,
    `--remote-debugging-port=${port}`,
    "about:blank",
  ],
  { stdio: "ignore" },
);
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
const words =
  lang === "zh"
    ? {
        general: "常规",
        appearance: "外观",
        refresh: "刷新状态",
        continue: "继续",
        install: "安装…",
        update: "更新…",
        repair: "修复…",
        remove: "停用…",
      }
    : {
        general: "General",
        appearance: "Appearance",
        refresh: "Refresh status",
        continue: "Continue",
        install: "Install…",
        update: "Update…",
        repair: "Repair…",
        remove: "Disable…",
      };
let ws;
const checks = [];
try {
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await delay(100);
    target = await fetch(`http://127.0.0.1:${port}/json`)
      .then((r) => r.json())
      .then((l) => l.find((t) => t.type === "page"))
      .catch(() => null);
  }
  assert.ok(target, "Chrome started");
  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener("open", r, { once: true }));
  let id = 0;
  const pending = new Map();
  ws.addEventListener("message", (e) => {
    const m = JSON.parse(e.data);
    if (m.method === "Runtime.exceptionThrown")
      console.error(m.method, JSON.stringify(m.params));
    if (pending.has(m.id)) {
      pending.get(m.id)(m);
      pending.delete(m.id);
    }
  });
  const send = (method, params = {}) =>
    new Promise((resolve) => {
      pending.set(++id, resolve);
      ws.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) => {
    const r = (
      await send("Runtime.evaluate", {
        expression,
        returnByValue: true,
        awaitPromise: true,
      })
    ).result;
    assert.ok(!r.exceptionDetails, JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  await send("Network.enable");
  await send("Runtime.enable");
  await send("Page.bringToFront");
  await send("Emulation.setFocusEmulationEnabled", { enabled: true });
  await send("Emulation.setDeviceMetricsOverride", {
    width: 640,
    height: 780,
    deviceScaleFactor: 1,
    mobile: false,
  });
  const until = async (fn) => {
    for (let i = 0; i < 300; i++) {
      if (await fn()) return;
      await delay(50);
    }
    console.error(
      "timeout",
      await evaluate(
        "({url:location.href,counts:window.helperFixture?.counts,body:document.body.innerText})",
      ),
    );
    throw Error("Preview condition timed out");
  };
  const counts = () => evaluate("window.helperFixture?.counts");
  const click = async (text) => {
    const box = await evaluate(
      `(() => {const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===${JSON.stringify(text)});if(!b||b.disabled)return null;b.scrollIntoView({block:'center'});const r=b.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`,
    );
    assert.ok(box, `enabled button ${text}`);
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
      await send("Input.dispatchMouseEvent", {
        type,
        ...box,
        button: "left",
        clickCount: 1,
      });
  };
  const refreshReady = async () =>
    (await counts())?.pendingReads === 0 &&
    (await evaluate(
      `!![...document.querySelectorAll('button')].find(b=>b.textContent.trim()===${JSON.stringify(words.refresh)}&&!b.disabled)`,
    ));
  async function load(query = "") {
    await send("Page.navigate", {
      url: `${base}/helper-settings-panel.html?lang=${lang}&${query}`,
    });
    await until(async () => !!(await counts()));
    await delay(850);
    await until(refreshReady);
  }
  for (const [timing, query] of [
    ["sequential", "preferences-delay=350"],
    ["overlapping", "status-delay=350"],
  ]) {
    await load(query);
    const initial = await counts();
    if (mode === "reproduce") {
      checks.push({ timing, initial });
      continue;
    }
    assert.equal(initial.reads, 1, `${timing} initial status read`);
    assert.equal(initial.operations, 0);
    await evaluate("window.dispatchEvent(new Event('focus')); true");
    await until(async () => (await counts()).reads === 2);
    await until(refreshReady);
    await click(words.refresh);
    await until(async () => (await counts()).reads === 3);
    await until(refreshReady);
    await click(words.appearance);
    await delay(100);
    await click(words.general);
    await until(async () => (await counts()).reads === 4);
    await until(refreshReady);
    assert.equal((await counts()).operations, 0);
    checks.push({
      timing,
      initial,
      afterFocusManualAndRemount: await counts(),
    });
  }
  if (mode !== "reproduce") {
    for (const [state, action] of [
      ["absent", "install"],
      ["old", "update"],
      ["mixed", "repair"],
      ["current", "remove"],
    ]) {
      await load(`state=${state}`);
      await click(words[action]);
      assert.equal((await counts()).operations, 0, "consent before operation");
      await click(words.continue);
      await until(async () => (await counts()).reads === 2);
      await until(refreshReady);
      assert.equal((await counts()).operations, 1);
      checks.push({ action, counts: await counts() });
    }
    for (const outcome of ["cancel", "error"]) {
      await load(`outcome=${outcome}`);
      await click(words.install);
      await click(words.continue);
      await until(async () => (await counts()).reads === 2);
      await until(refreshReady);
      assert.equal((await counts()).operations, 1);
      checks.push({ outcome, counts: await counts() });
    }
    await load("state=query-error");
    assert.equal((await counts()).reads, 1);
    assert.ok(await evaluate("!!document.querySelector('[role=alert]')"));
    await evaluate("window.helperFixture.recover(); true");
    await click(words.refresh);
    await until(async () => (await counts()).reads === 2);
    await until(refreshReady);
    assert.equal(
      await evaluate("!!document.querySelector('[role=alert]')"),
      false,
    );
    checks.push({ errorRecovered: await counts() });
    await load("state=old");
    const bounds = await evaluate(
      `(() => {const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===${JSON.stringify(words.update)});b.scrollIntoView({block:'center'});const p=b.closest('.setup');return {fits:p.scrollWidth<=p.clientWidth+1,headerTitles:document.querySelectorAll('th [title]').length};})()`,
    );
    assert.ok(bounds.fits);
    checks.push({ layout: bounds });
    const png = (await send("Page.captureScreenshot", { format: "png" })).result
      .data;
    writeFileSync(
      join(out, `helper-settings-${lang}.png`),
      Buffer.from(png, "base64"),
    );
  }
  writeFileSync(
    join(out, `helper-settings-${lang}${mode ? "-" + mode : ""}.json`),
    JSON.stringify({ synthetic: true, checks }, null, 2),
  );
  console.log(JSON.stringify(checks, null, 2));
} finally {
  ws?.close();
  chrome.kill();
  await server.close();
  await new Promise((resolve) =>
    chrome.exitCode !== null ? resolve() : chrome.once("exit", resolve),
  );
  rmSync(profile, {
    recursive: true,
    force: true,
    maxRetries: 5,
    retryDelay: 100,
  });
}
