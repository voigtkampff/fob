// The real main window route with mocked readings and disk health replies.
import { mount } from "svelte";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import Page from "../../src/routes/+page.svelte";
import { REPLIES, stats } from "./disk-health-fixtures.js";
import { DEFAULT_CONFIG } from "../../src/lib/definitions/settings";
import { mergePreferences } from "../../src/lib/utils/settings-patch.js";

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
/** Every command the page sends, in order. */
/** @type {{ command: string, args: unknown }[]} */
const ipcLog = [];
Object.defineProperty(window, "ipcLog", { value: ipcLog });
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
// ?live: every read is a new frame whose readings move (deterministic), with
// a growing history, the way a running app's are; for paint and update
// checks. Without it every read returns the same frame.
const live = params.has("live");
let tick = 0;
let livePaused = false;
/** @type {any} */
let lastLive = null;
/** @type {import("../../src/lib/types").ResourceSample[]} */
const history = [];
const liveSnapshot = () => {
  if (livePaused && lastLive) return { ...lastLive, paused: true };
  tick++;
  const wave = (/** @type {number} */ k) => (Math.sin(tick * 0.9 + k) + 1) / 2;
  history.push({
    timestamp: Date.now(),
    interval: 1000,
    cpu: 10 + 30 * wave(0),
    memory: 40 + 5 * wave(1),
    received: 1e5 * wave(2),
    sent: 4e4 * wave(3),
    gpus: {},
    disk_read: 2e6 * wave(4),
    disk_write: 1e6 * wave(5),
  });
  if (history.length > 300) history.shift();
  lastLive = {
    ...snapshot,
    frame: {
      ...snapshot.frame,
      captured_at: Date.now(),
      sequence: tick + 1,
      processes: processes.map((p, i) => ({
        ...p,
        cpu_usage:
          p.cpu_usage == null
            ? null
            : Math.round(p.cpu_usage * (0.5 + wave(i)) * 10) / 10,
        disk_usage: [Math.round(4096 * wave(i + 1)), 0],
      })),
      stats: {
        ...stats,
        cpu_usage: [10 + 30 * wave(0), 15 + 20 * wave(1)],
        network_rx_bytes: (stats.network_rx_bytes ?? 0) + tick * 1e5,
        network_tx_bytes: (stats.network_tx_bytes ?? 0) + tick * 4e4,
      },
    },
    history: [...history],
  };
  return lastLive;
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
// The native side merges a patch the same way (preferences.rs `merge`),
// replacing column widths whole.
const merge = mergePreferences;
mockIPC(async (command, args) => {
  ipcLog.push({ command, args });
  if (command === "read_preferences") return { config, revision, ready: true };
  if (command === "update_preferences") {
    const patch = /** @type {any} */ (args).patch;
    config = merge(config, patch);
    if (persist)
      sessionStorage.setItem("preview-config", JSON.stringify(config));
    return { config, revision: ++revision, ready: true };
  }
  if (command === "plugin:event|listen") return 1;
  if (command === "get_processes") {
    const result = live ? liveSnapshot() : snapshot;
    return {
      ...result,
      history: {
        reset: true,
        samples: result.history,
        cursor: null,
        first_timestamp: result.history[0]?.timestamp ?? null,
      },
    };
  }
  // The poll skips a hidden or minimized window; with ?live it is shown.
  if (live && command === "plugin:window|is_visible") return true;
  if (live && command === "set_monitor_paused") {
    livePaused = /** @type {any} */ (args).paused;
    return null;
  }
  if (live && command === "plugin:window|is_minimized") return false;
  if (command === "disk_health") {
    calls.push(performance.now());
    if (state === "loading") return new Promise(() => {});
    if (state === "failed") throw "WMI provider is not available";
    return REPLIES[state];
  }
  return null;
});
mount(Page, { target: document.body });
