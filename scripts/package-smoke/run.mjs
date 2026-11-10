// Runs the unpacked Fob zip on this Windows desktop and checks it the way a
// user would. usage: node run.mjs <fob dir> <evidence dir>
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [fobDir, out] = process.argv.slice(2);
const PORT = 9222;
const WIN32 = join(import.meta.dirname, "win32.ps1");
const configDir = join(process.env.APPDATA, "com.hcb.fob");
// WebView2 keeps page storage (the last tab, theme) here between runs.
const webviewDir = join(process.env.LOCALAPPDATA, "com.hcb.fob");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const checks = [];
const report = { fobDir, checks, runs: [] };
const check = (name, pass, detail) => {
  checks.push({ name, pass: !!pass, detail });
  console.log(
    `${pass ? "ok  " : "FAIL"} ${name}${detail === undefined ? "" : ` ${JSON.stringify(detail)}`}`,
  );
  return !!pass;
};
const native = (action, arg = "", file = "") =>
  JSON.parse(
    execFileSync(
      "powershell",
      [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        WIN32,
        action,
        String(arg),
        file,
      ],
      {
        encoding: "utf8",
      },
    ) || "null",
  );
const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
const until = async (fn, ms, step = 250) => {
  for (const end = Date.now() + ms; Date.now() < end; await sleep(step)) {
    const v = await fn();
    if (v) return v;
  }
  return null;
};

async function cdp() {
  const pages = await until(async () => {
    try {
      const list = await (
        await fetch(`http://127.0.0.1:${PORT}/json/list`)
      ).json();
      return list.length ? list : null;
    } catch {
      return null;
    }
  }, 30000);
  if (!pages) return null;
  for (const page of pages.filter((p) => p.type === "page")) {
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((ok, fail) => {
      ws.onopen = ok;
      ws.onerror = fail;
    });
    let id = 0;
    const waiting = new Map();
    ws.onclose = () => {
      for (const ok of waiting.values()) ok({});
      waiting.clear();
    };
    ws.onmessage = (m) => {
      const msg = JSON.parse(m.data);
      waiting.get(msg.id)?.(msg);
      waiting.delete(msg.id);
    };
    const send = (method, params = {}) =>
      new Promise((ok) => {
        waiting.set(++id, ok);
        ws.send(JSON.stringify({ id, method, params }));
      });
    const evaluate = async (expression) =>
      (
        await send("Runtime.evaluate", {
          expression,
          returnByValue: true,
          awaitPromise: true,
        })
      ).result?.result?.value;
    const isMain = await until(
      () => evaluate("!!document.querySelector('#resource-trigger-storage')"),
      20000,
    );
    if (!isMain) {
      ws.close();
      continue;
    }
    const click = async (selector) => {
      const box = await evaluate(
        `(() => { const e = [...document.querySelectorAll(${JSON.stringify(selector)})].find((e) => e.getClientRects().length); if (!e) return null; e.scrollIntoView({ block: "center" }); const r = e.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
      );
      if (!box) return false;
      for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
        await send("Input.dispatchMouseEvent", {
          type,
          ...box,
          button: "left",
          clickCount: 1,
        });
      return true;
    };
    const shot = async (name) =>
      writeFileSync(
        join(out, name),
        Buffer.from(
          (await send("Page.captureScreenshot", { format: "png" })).result.data,
          "base64",
        ),
      );
    return { page: page.url, evaluate, click, shot, close: () => ws.close() };
  }
  return null;
}

async function launch(tag, behavior) {
  const lingering = await until(
    () => native("webview-processes", "com.hcb.fob") === 0,
    30000,
    1000,
  );
  check(`${tag}: no WebView2 process of an earlier launch is left`, lingering);
  for (const dir of [configDir, webviewDir])
    rmSync(dir, {
      recursive: true,
      force: true,
      maxRetries: 20,
      retryDelay: 500,
    });
  mkdirSync(configDir, { recursive: true });
  writeFileSync(join(configDir, "settings.json"), JSON.stringify({ behavior }));
  const child = spawn(join(fobDir, "fob.exe"), [], {
    cwd: fobDir,
    env: {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`,
    },
    stdio: "ignore",
  });
  const exit = new Promise((ok) =>
    child.on("exit", (code, signal) => ok({ code, signal })),
  );
  const run = { tag, behavior, pid: child.pid };
  report.runs.push(run);
  const main = () =>
    native("windows", child.pid).find(
      (w) =>
        w.title === "Fob" &&
        w.visible &&
        !w.iconic &&
        w.rect[2] >= 400 &&
        w.rect[3] >= 300,
    );
  run.window = await until(main, 60000, 500);
  run.allWindows = native("windows", child.pid);
  run.elevation = native("elevated", child.pid);
  check(
    `${tag}: main window "Fob" is visible, not minimized, at a usable size`,
    run.window,
    run.window?.rect,
  );
  run.page = run.window && (await cdp());
  check(
    `${tag}: the main window's page rendered the dashboard`,
    run.page,
    run.page?.page,
  );
  if (run.window) {
    run.windowShot = native(
      "window-shot",
      run.window.hwnd,
      join(out, `${tag}-window.png`),
    );
    run.screenShot = native("screen-shot", "", join(out, `${tag}-screen.png`));
  }
  return { run, child, exit };
}

async function waitExit(tag, how, child, exit, ms) {
  const done = await Promise.race([exit, sleep(ms).then(() => null)]);
  check(
    `${tag}: ${how} ends Fob with code 0 within ${ms / 1000} s`,
    done && done.code === 0,
    done,
  );
  if (!done && alive(child.pid)) {
    child.kill();
    check(
      `${tag}: no forced cleanup was needed`,
      false,
      "killed after timeout",
    );
  }
}

mkdirSync(out, { recursive: true });
report.runner = {
  os: execFileSync(
    "pwsh",
    [
      "-NoProfile",
      "-Command",
      "(Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber | ConvertTo-Json -Compress)",
    ],
    { encoding: "utf8" },
  ).trim(),
  webview2: execFileSync(
    "pwsh",
    [
      "-NoProfile",
      "-Command",
      "(Get-ItemProperty 'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue).pv",
    ],
    { encoding: "utf8" },
  ).trim(),
  session: execFileSync(
    "pwsh",
    [
      "-NoProfile",
      "-Command",
      '"$([System.Diagnostics.Process]::GetCurrentProcess().SessionId) $([Environment]::UserInteractive)"',
    ],
    { encoding: "utf8" },
  ).trim(),
  disks: execFileSync(
    "pwsh",
    [
      "-NoProfile",
      "-Command",
      "Get-PhysicalDisk | Select-Object FriendlyName,BusType,MediaType | ConvertTo-Json -Compress",
    ],
    { encoding: "utf8" },
  ).trim(),
};
console.log("runner", JSON.stringify(report.runner));

try {
  const first = await launch("en", {
    language: "en",
    startInTray: false,
    keepRunning: true,
    menuBarEnabled: true,
  });
  const { page, window } = first.run;
  if (page) {
    check(
      "en: storage card opens its side panel",
      (await page.click("#resource-trigger-storage")) &&
        (await until(
          () =>
            page.evaluate(
              "!!document.querySelector('[data-slot=\"sheet-content\"]')",
            ),
          8000,
        )),
    );
    await page.shot("en-storage-panel.png");
    check(
      "en: the panel's disk health link opens the Disk detail",
      (await page.click(
        '[data-slot="sheet-content"] button:has(.lucide-heart-pulse)',
      )) &&
        (await until(
          () => page.evaluate("!!document.getElementById('disk-health-title')"),
          8000,
        )),
    );
    const health = await until(
      () =>
        page.evaluate(`(() => {
      const p = document.querySelector('.panel.health');
      if (!p || (p.innerText.includes('Reading disk health') && !p.querySelector('.disk'))) return null;
      const disks = [...p.querySelectorAll('.disk')];
      if (!disks.length && !p.querySelector('.note')) return null;
      return { heading: document.querySelector('main .head h2')?.textContent.trim(), disks: disks.map((d) => d.innerText.replace(/\\s+/g, ' ').trim()), notes: [...p.querySelectorAll(':scope > .note')].map((n) => n.textContent.trim()), text: p.innerText };
    })()`),
      90000,
      1000,
    );
    first.run.health = health;
    check(
      "en: Disk detail lists at least one physical disk read on this machine",
      health?.disks.length >= 1,
      health,
    );
    check(
      "en: no untranslated text keys in the Disk health section",
      health && !/\b(dh|nvme|ata)\.[a-z_]+/.test(health.text),
    );
    await page.shot("en-disk-detail.png");
    native("window-shot", window.hwnd, join(out, "en-disk-detail-window.png"));
    native("close", window.hwnd);
    await sleep(3000);
    const after = native("windows", first.child.pid).find(
      (w) => w.hwnd === window.hwnd,
    );
    check(
      "en: closing the window keeps Fob running in the tray",
      alive(first.child.pid) && after && !after.visible,
      after,
    );
    await page
      .evaluate("window.__TAURI_INTERNALS__.invoke('quit_app')")
      .catch(() => {});
    page.close();
  }
  await waitExit(
    "en",
    "the app's quit command, sent to the hidden window,",
    first.child,
    first.exit,
    20000,
  );

  const second = await launch("zh", {
    language: "zh",
    startInTray: false,
    keepRunning: true,
    menuBarEnabled: true,
  });
  if (second.run.page) {
    const page = second.run.page;
    check(
      "zh: the title bar menu opens",
      (await page.click('button[aria-label="Fob 菜单"]')) &&
        (await until(
          () =>
            page.evaluate(
              "[...document.querySelectorAll('[role=menuitem]')].some((e) => e.textContent.includes('退出 Fob'))",
            ),
          5000,
        )),
    );
    await page.shot("zh-menu.png");
    await page.evaluate(
      "[...document.querySelectorAll('[role=menuitem]')].find((e) => e.textContent.includes('退出 Fob'))?.setAttribute('data-smoke', 'quit')",
    );
    check("zh: Quit Fob is clicked", await page.click('[data-smoke="quit"]'));
    page.close();
  }
  await waitExit(
    "zh",
    "clicking Quit Fob in the title bar menu",
    second.child,
    second.exit,
    20000,
  );
} catch (error) {
  check("smoke completed", false, String(error?.stack ?? error));
}
for (const run of report.runs)
  if (alive(run.pid)) {
    process.kill(run.pid);
    check(
      `${run.tag}: Fob was not left running`,
      false,
      "killed during cleanup",
    );
  }
report.pass = checks.length > 0 && checks.every((c) => c.pass);
writeFileSync(join(out, "run.json"), JSON.stringify(report, null, 2));
process.exit(report.pass ? 0 : 1);
