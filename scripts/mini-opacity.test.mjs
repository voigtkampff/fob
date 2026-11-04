import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  miniOpacity,
  OPACITY_MIN,
  OPACITY_MAX,
} from "../src/lib/utils/mini-opacity.js";

test("mini opacity is a whole percent between 40 and 100, default 100", () => {
  assert.equal(miniOpacity(undefined), 100);
  assert.equal(miniOpacity("junk"), 100);
  assert.equal(miniOpacity(70), 70);
  assert.equal(miniOpacity("70"), 70);
  assert.equal(miniOpacity(12), 40);
  assert.equal(miniOpacity(140), 100);
  assert.equal(miniOpacity(72.6), 73);
  // Same range as the backend's validation.
  const rust = readFileSync("src-tauri/src/preferences.rs", "utf8");
  assert.match(
    rust,
    new RegExp(`"miniOpacity" => .*\\(${OPACITY_MIN}\\.\\.=${OPACITY_MAX}\\)`),
  );
});

test("one slider component serves Settings and the mini window", () => {
  const settings = readFileSync(
    "src/lib/components/SettingsWindow.svelte",
    "utf8",
  );
  const mini = readFileSync(
    "src/lib/components/MenuBarOverview.svelte",
    "utf8",
  );
  for (const page of [settings, mini]) assert.match(page, /<OpacitySlider/);
  // The fade is applied to backgrounds only (text keeps full opacity).
  const css = readFileSync("src/app.css", "utf8");
  assert.match(css, /--mini-alpha/);
  assert.doesNotMatch(css + mini, /\bopacity:\s*var\(--mini-alpha/);
  // A transparent window is what lets the backgrounds fade.
  const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  const overview = conf.app.windows.find((w) => w.label === "overview");
  assert.equal(overview.transparent, true);
});
