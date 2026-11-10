// The real main window route with mocked readings and disk health replies.
import { mount } from "svelte";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import Page from "../../src/routes/+page.svelte";
import { REPLIES, stats } from "./disk-health-fixtures.js";
import { DEFAULT_CONFIG } from "../../src/lib/definitions/settings";

const params = new URLSearchParams(location.search);
const lang = params.get("lang") === "zh" ? "zh" : "en";
const state = params.get("state") ?? "ok";
const config = {
  ...DEFAULT_CONFIG,
  behavior: { ...DEFAULT_CONFIG.behavior, language: lang },
};
/** @type {number[]} */
const calls = [];
Object.defineProperty(window, "diskHealthCalls", { value: calls });
Object.defineProperty(window, "isTauri", { value: true, configurable: true });
const snapshot = {
  frame: {
    captured_at: Date.now(),
    sequence: 1,
    processes: [],
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
// @ts-expect-error The test transport deliberately returns concrete fixture values.
mockIPC(async (command) => {
  if (command === "read_preferences")
    return { config, revision: 1, ready: true };
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
