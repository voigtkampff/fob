import { mount } from "svelte";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import CpuViewsPreview from "./CpuViewsPreview.svelte";
import { themeStore } from "../../src/lib/stores/theme";
import { settingsStore } from "../../src/lib/stores/settings";
import { DEFAULT_CONFIG } from "../../src/lib/definitions/settings";

const params = new URLSearchParams(location.search);
const lang = params.get("lang") === "zh" ? "zh" : "en";
const config = {
  ...DEFAULT_CONFIG,
  behavior: { ...DEFAULT_CONFIG.behavior, language: lang },
};
Object.defineProperty(window, "isTauri", { value: true, configurable: true });
mockWindows("main");
// @ts-expect-error The test transport deliberately returns concrete fixture values.
mockIPC(async (command) => {
  if (command === "read_preferences")
    return { config, revision: 1, ready: true };
  if (command === "plugin:event|listen") return 1;
  if (command === "get_process_icons") return [];
  return null;
});
themeStore.init();
await settingsStore.init();
const target = document.createElement("div");
document.body.append(target);
mount(CpuViewsPreview, {
  target,
  props: { cpu: params.get("cpu") ?? "intel" },
});
