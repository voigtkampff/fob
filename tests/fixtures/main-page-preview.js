// The real main window route with mocked readings and disk health replies.
import { mount } from "svelte";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import Page from "../../src/routes/+page.svelte";
import { REPLIES, stats } from "./disk-health-fixtures.js";
import { DEFAULT_CONFIG } from "../../src/lib/definitions/settings";

const params = new URLSearchParams(location.search);
const lang = params.get("lang") === "zh" ? "zh" : "en";
const state = params.get("state") ?? "ok";
// ?saved keeps preferences in sessionStorage across reloads, the way the
// settings file does; it is the stored config as the page last saved it.
const persist = params.has("saved");
const stored = persist
  ? JSON.parse(sessionStorage.getItem("preview-config") ?? "null")
  : null;
let config = stored ?? {
  ...DEFAULT_CONFIG,
  behavior: { ...DEFAULT_CONFIG.behavior, language: lang },
};
let revision = 1;
const MB = 1024 * 1024;
const now = Math.floor(Date.now() / 1000);
/** @type {(pid: number, ppid: number, name: string, cpu: number, mem: number, user?: string) => import("../../src/lib/types").Process} */
const proc = (pid, ppid, name, cpu, mem, user = "sample-user") => ({
  pid,
  ppid,
  name,
  cpu_usage: cpu,
  gpu_usage: 0,
  memory_usage: mem * MB,
  status: "Running",
  user,
  command: name,
  environ: [],
  root: "",
  virtual_memory: mem * 3 * MB,
  start_time: now - 3600,
  run_time: 3600,
  disk_read: 0,
  disk_write: null,
  gpu_memory: null,
  session_id: 1,
  ports: [],
});
// ?procs: generic mocked processes, a long name and a three level family.
const processes = params.has("procs")
  ? [
      proc(4, 0, "System", 0.3, 1, "SYSTEM"),
      proc(100, 4, "Example.Host.exe", 6.5, 320, "LOCAL SERVICE"),
      proc(101, 100, "Example.Worker.exe", 3.2, 210),
      proc(102, 101, "Example.Renderer.exe", 1.1, 120),
      proc(
        200,
        4,
        "Example Very Long Process Name Used For Column Width Checks Service Host.exe",
        0.8,
        90,
        // An owner with no account keeps its (made-up) SID.
        "S-1-5-21-1000000001-1000000002-1000000003-1999",
      ),
      proc(300, 4, "Editor.exe", 12.4, 840),
      proc(301, 4, "Browser.exe", 4.2, 1500),
    ]
  : [];
/** @type {number[]} */
const calls = [];
Object.defineProperty(window, "diskHealthCalls", { value: calls });
Object.defineProperty(window, "isTauri", { value: true, configurable: true });
const snapshot = {
  frame: {
    captured_at: Date.now(),
    sequence: 1,
    processes,
    stats,
    gpus: [],
  },
  history: [],
  paused: false,
  interval: 1000,
  error: null,
};
// The 5 minute health refresh timers alive right now, so the check can see
// that closing the panel or leaving the Disk detail stops polling.
const health = new Set();
const setInterval = window.setInterval.bind(window);
const clearInterval = window.clearInterval.bind(window);
/** @type {(handler: TimerHandler, ms?: number, ...rest: unknown[]) => number} */
const tracked = (handler, ms, ...rest) => {
  const id = setInterval(handler, ms, ...rest);
  if (ms === 5 * 60 * 1000) health.add(id);
  return id;
};
window.setInterval = /** @type {typeof window.setInterval} */ (tracked);
/** @type {(id?: number) => void} */
const untrack = (id) => {
  if (id !== undefined) health.delete(id);
  clearInterval(id);
};
window.clearInterval = /** @type {typeof window.clearInterval} */ (untrack);
Object.defineProperty(window, "healthTimers", { value: () => health.size });
mockWindows("main");
/** Patches carry only the changed leaves; arrays replace whole. */
/** @type {(base: any, patch: any) => any} */
const merge = (base, patch) =>
  Object.fromEntries(
    [...new Set([...Object.keys(base), ...Object.keys(patch)])].map((key) => {
      const next = patch[key];
      if (next === undefined) return [key, base[key]];
      const nested =
        next && typeof next === "object" && !Array.isArray(next) && base[key];
      return [key, nested ? merge(base[key], next) : next];
    }),
  );
// @ts-expect-error The test transport deliberately returns concrete fixture values.
mockIPC(async (command, args) => {
  if (command === "read_preferences") return { config, revision, ready: true };
  if (command === "update_preferences") {
    const patch = /** @type {any} */ (args).patch;
    config = merge(config, patch);
    if (persist)
      sessionStorage.setItem("preview-config", JSON.stringify(config));
    return { config, revision: ++revision, ready: true };
  }
  if (command === "plugin:event|listen") return 1;
  if (command === "get_processes") return snapshot;
  if (command === "disk_health") {
    calls.push(performance.now());
    if (state === "loading") return new Promise(() => {});
    if (state === "failed") throw "WMI provider is not available";
    return REPLIES[state];
  }
  return null;
});
mount(Page, { target: document.body });
