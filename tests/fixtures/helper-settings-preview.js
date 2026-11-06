import { mount } from "svelte";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import SettingsWindow from "../../src/lib/components/SettingsWindow.svelte";
import { themeStore } from "../../src/lib/stores/theme";
import { mergePreferences } from "../../src/lib/utils/settings-patch.js";
import { DEFAULT_CONFIG } from "../../src/lib/definitions/settings";
import "../../src/app.css";

const params = new URLSearchParams(location.search);
const lang = params.get("lang") === "zh" ? "zh" : "en";
const scenario = params.get("state") ?? "absent";
const outcome = params.get("outcome") ?? "success";
/** @type {import("../../src/lib/types").SensorSetupStatus} */
const absent = {
  supported: true,
  bundled: true,
  pawnio_installed: false,
  installed: false,
  task: "missing",
  update_available: false,
  running: false,
  fake: true,
  installed_version: null,
  bundled_version: "0.8.2",
  busy: false,
};
/** @type {import("../../src/lib/types").SensorSetupStatus} */
const current = {
  ...absent,
  installed: true,
  pawnio_installed: true,
  task: "ok",
  running: true,
  installed_version: "0.8.2",
};
/** @type {Record<string, import("../../src/lib/types").SensorSetupStatus>} */
const statuses = {
  absent,
  old: { ...current, installed_version: "0.8.1", update_available: true },
  mixed: { ...current, update_available: true },
  current,
  stopped: { ...current, running: false },
  busy: { ...absent, busy: true },
  missing: { ...absent, bundled: false },
};
let status = statuses[scenario] ?? absent;
let operations = 0;
let reads = 0;
let config = {
  ...DEFAULT_CONFIG,
  behavior: { ...DEFAULT_CONFIG.behavior, language: lang },
};
let revision = 1;
Object.defineProperty(window, "isTauri", { value: true, configurable: true });
mockWindows("settings");
// Tauri v2 declares a generic return T even for concrete mock responses.
// @ts-expect-error The test transport deliberately returns concrete fixture values.
mockIPC(async (command, args) => {
  if (command === "sensor_setup_status") {
    reads++;
    const counter = document.querySelector("#fixture-reads");
    if (counter) counter.textContent = String(reads);
    if (scenario === "query-error") throw "Fixture status query unavailable";
    return status;
  }
  if (command === "sensor_setup") {
    operations++;
    const counter = document.querySelector("#fixture-count");
    if (counter) counter.textContent = String(operations);
    await new Promise((resolve) => setTimeout(resolve, 1500));
    if (outcome === "cancel") throw { kind: "cancelled" };
    if (outcome === "error")
      throw {
        kind: "failed",
        message:
          "Fixture copy failed at C:\\Program Files\\Fob\\sensors\\fob-mft.exe. " +
          "Long error detail ".repeat(30),
      };
    status =
      args && "enable" in args && args.enable === true ? current : absent;
    return status;
  }
  if (command === "plugin:event|listen") return 1;
  if (command === "settings_pane") return "general";
  if (command === "get_mini_position_saved") return false;
  if (command === "get_hotkey") return { combo: "", error: null, failed: "" };
  if (command === "read_preferences") return { config, revision, ready: true };
  if (command === "update_preferences" && args && "patch" in args) {
    config = mergePreferences(config, args.patch);
    return { config, revision: ++revision, ready: true };
  }
  return null;
});
const banner = document.createElement("div");
banner.style.cssText =
  "font:12px sans-serif;padding:8px;background:#fef3c7;color:#111;position:sticky;top:0;z-index:9999";
banner.textContent = `Browser fixture only. Mocked Windows helper, no installation or UAC. State ${scenario}, outcome ${outcome}. Setup calls `;
const counter = document.createElement("span");
counter.id = "fixture-count";
counter.textContent = "0";
banner.append(counter, ". Status reads ");
const readCounter = document.createElement("span");
readCounter.id = "fixture-reads";
readCounter.textContent = "0";
banner.append(readCounter, ". ");
for (const name of [...Object.keys(statuses), "query-error"]) {
  const a = document.createElement("a");
  a.href = `?lang=${lang}&state=${name}&outcome=${outcome}`;
  a.textContent = name + " ";
  banner.append(a);
}
for (const name of ["en", "zh"]) {
  const a = document.createElement("a");
  a.href = `?lang=${name}&state=${scenario}&outcome=${outcome}`;
  a.textContent = name + " ";
  banner.append(a);
}
for (const name of ["success", "cancel", "error"]) {
  const a = document.createElement("a");
  a.href = `?lang=${lang}&state=${scenario}&outcome=${name}`;
  a.textContent = name + " ";
  banner.append(a);
}
themeStore.init();
document.body.append(banner);
const target = document.createElement("div");
document.body.append(target);
mount(SettingsWindow, { target });
