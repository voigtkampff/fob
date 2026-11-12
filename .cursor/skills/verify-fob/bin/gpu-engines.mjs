// gpu-engines.mjs <zh|en> <evidence-dir>: serves the mocked GPU detail
// (engine list and “about these readings”, production CSS). Fails unless
// official Chinese names, raw English fallbacks, localized source notes
// and hover titles are present. Mocked readings only; not Windows evidence.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync, copyFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import assert from "node:assert/strict";
import { createServer } from "vite";

const [lang = "zh", out = "/workspace/scratch/verify/evidence"] =
  process.argv.slice(2);
mkdirSync(out, { recursive: true });
const publicOut = "/workspace";
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
const port = 9320 + (lang === "zh" ? 1 : 2);
const W = 1400;
const H = 1100;
const chrome = spawn(
  "google-chrome",
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    "--hide-scrollbars",
    `--remote-debugging-port=${port}`,
    `--window-size=${W},${H}`,
    "about:blank",
  ],
  { stdio: "ignore" },
);

const expected =
  lang === "zh"
    ? {
        labels: [
          "三维引擎",
          "复制引擎",
          "LegacyOverlay",
          "光流加速器 0",
          "Security_1",
          "VR",
          "视频解码 1",
          "视频编码",
          "Compute_0",
        ],
        absent: ["utilization from", "NVIDIA NVML driver;"],
        note: /NVIDIA NVML 驱动；GPU 占用取自 PDH GPU 引擎计数器，与进程列表同一取样/,
        hover: "NVIDIA 光流硬件，用于运动估计，例如帧生成和视频处理",
        hoverPick: "光流加速器 0",
      }
    : {
        labels: [
          "3D",
          "Copy",
          "LegacyOverlay",
          "OFA_0",
          "Security_1",
          "VR",
          "VideoDecode_1",
          "VideoEncode",
          "Compute_0",
        ],
        absent: ["三维引擎", "光流加速器", "同一取样"],
        note: /NVIDIA NVML driver; GPU % from PDH GPU engine counters, same sample as the process list/,
        hover:
          "NVIDIA optical flow hardware for motion estimation, such as frame generation and video processing",
        hoverPick: "OFA_0",
      };

try {
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await new Promise((r) => setTimeout(r, 200));
    target = await fetch(`http://127.0.0.1:${port}/json`)
      .then((r) => r.json())
      .then((list) => list.find((t) => t.type === "page"))
      .catch(() => undefined);
  }
  if (!target) throw new Error("Chrome debug port did not expose a page");
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
    const m = await send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (m.result?.exceptionDetails) {
      throw new Error(m.result.exceptionDetails.text ?? "evaluate failed");
    }
    return m.result.result.value;
  };
  await send("Emulation.setDeviceMetricsOverride", {
    width: W,
    height: H,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await send("Page.enable");
  await send("Page.navigate", {
    url: `${base}/gpu-engines-panel.html?lang=${lang}`,
  });
  await send("Page.loadEventFired");
  for (let i = 0; i < 40; i++) {
    const ready = await evaluate(
      `document.querySelectorAll("#wide [data-engine]").length >= 9`,
    );
    if (ready) break;
    await new Promise((r) => setTimeout(r, 200));
  }

  const read = () =>
    evaluate(`(() => {
      const text = (el) => el?.textContent.replace(/\\s+/g, " ").trim() ?? "";
      const labelsOf = (root) =>
        [...root.querySelectorAll("[data-engine]")].map(text);
      const noteOf = (root) => {
        const title = [...root.querySelectorAll("p")].find((el) =>
          /关于这些读数|About these readings/.test(el.textContent),
        );
        const body = title?.nextElementSibling;
        return text(body ?? title);
      };
      const wide = document.querySelector("#wide");
      const narrow = document.querySelector("#narrow");
      const card = document.querySelector("#card");
      const labels = labelsOf(wide);
      const narrowLabels = labelsOf(narrow);
      const truncated = [...narrow.querySelectorAll("[data-engine]")].map((el) => ({
        text: text(el),
        title: el.getAttribute("title"),
        truncated: el.scrollWidth > el.clientWidth + 1,
      }));
      return {
        labels,
        narrowLabels,
        cardLabels: labelsOf(card),
        note: noteOf(wide),
        truncated,
        titles: [...wide.querySelectorAll("[data-engine]")].map((el) => ({
          text: text(el),
          title: el.getAttribute("title"),
        })),
      };
    })()`);

  const info = await read();
  assert.deepEqual(info.labels, expected.labels, JSON.stringify(info.labels));
  assert.deepEqual(info.narrowLabels, expected.labels);
  assert.deepEqual(info.cardLabels, expected.labels);
  assert.match(info.note, expected.note);
  for (const bit of expected.absent) {
    assert.doesNotMatch(info.note, new RegExp(bit.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
  const hover = info.titles.find((row) => row.text === expected.hoverPick);
  assert.ok(hover, `missing hover target ${expected.hoverPick}`);
  assert.equal(hover.title, expected.hover);

  const shot = async (name) => {
    const png = (await send("Page.captureScreenshot", { format: "png" })).result
      .data;
    const file = resolve(out, name);
    writeFileSync(file, Buffer.from(png, "base64"));
    copyFileSync(file, resolve(publicOut, name));
    return file;
  };

  await shot(`gpu-engines-${lang}-wide.png`);

  // Show the hover title as a visible bubble (headless Chrome often skips the
  // native title tooltip) and screenshot the narrow column too.
  await evaluate(`(() => {
    const pick = ${JSON.stringify(expected.hoverPick)};
    const el = [...document.querySelectorAll("#wide [data-engine]")].find(
      (node) => node.textContent.trim() === pick,
    );
    if (!el) return false;
    const tip = document.createElement("div");
    tip.id = "hover-preview";
    tip.textContent = el.getAttribute("title") ?? "";
    const box = el.getBoundingClientRect();
    Object.assign(tip.style, {
      position: "fixed",
      left: box.left + "px",
      top: box.bottom + 6 + "px",
      maxWidth: "360px",
      padding: "6px 8px",
      background: "#111",
      color: "#fff",
      borderRadius: "4px",
      fontSize: "12px",
      lineHeight: "1.4",
      zIndex: "9999",
      boxShadow: "0 2px 8px rgba(0,0,0,.4)",
      pointerEvents: "none",
    });
    document.body.append(tip);
    el.scrollIntoView({ block: "center" });
    return true;
  })()`);
  await shot(`gpu-engines-${lang}-hover.png`);
  await evaluate(`document.getElementById("hover-preview")?.remove()`);

  await evaluate(`document.querySelector("#narrow")?.scrollIntoView()`);
  await shot(`gpu-engines-${lang}-narrow.png`);

  writeFileSync(
    resolve(out, `gpu-engines-${lang}.json`),
    JSON.stringify({ lang, ...info, synthetic: true }, null, 2),
  );
  console.log(`PASS: gpu engines preview (${lang})`);
} finally {
  chrome.kill();
  await server.close();
}
