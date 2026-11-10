import { mount } from "svelte";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import DiskHealthPreview from "./DiskHealthPreview.svelte";
import { REPLIES } from "./disk-health-fixtures.js";
import { themeStore } from "../../src/lib/stores/theme";
import { settingsStore } from "../../src/lib/stores/settings";
import { DEFAULT_CONFIG } from "../../src/lib/definitions/settings";

const params = new URLSearchParams(location.search);
const lang = params.get("lang") === "zh" ? "zh" : "en";
const state = params.get("state") ?? "ok";
const config = {
  ...DEFAULT_CONFIG,
  behavior: { ...DEFAULT_CONFIG.behavior, language: lang },
};
/** @type {string[]} */
const calls = [];
Object.defineProperty(window, "diskHealthCalls", { value: calls });
Object.defineProperty(window, "isTauri", { value: true, configurable: true });
mockWindows("main");
// @ts-expect-error The test transport deliberately returns concrete fixture values.
mockIPC(async (command) => {
  if (command === "read_preferences")
    return { config, revision: 1, ready: true };
  if (command === "plugin:event|listen") return 1;
  if (command === "disk_health") {
    calls.push(state);
    if (state === "loading") return new Promise(() => {});
    if (state === "failed") throw "WMI provider is not available";
    return REPLIES[state];
  }
  return null;
});
themeStore.init();
await settingsStore.init();
const target = document.createElement("div");
document.body.append(target);
mount(DiskHealthPreview, {
  target,
  props: { width: Number(params.get("width") ?? 980) },
});
