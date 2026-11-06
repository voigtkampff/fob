import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  miniOpacity,
  OPACITY_MIN,
  OPACITY_MAX,
  OPACITY_STEP,
} from "../src/lib/utils/mini-opacity.js";

test("mini opacity supports every whole percent from 0 through 100", () => {
  for (let percent = 0; percent <= 100; percent++) {
    assert.equal(miniOpacity(percent), percent);
    assert.equal(miniOpacity(String(percent)), percent);
  }
});

test("mini opacity clamps outside the range and rounds to the nearest percent", () => {
  for (const [input, expected] of [
    [-100, 0],
    [-1, 0],
    [-0.6, 0],
    [0.4, 0],
    [0.5, 1],
    [12, 12],
    [39, 39],
    [72.4, 72],
    [72.5, 73],
    [99.6, 100],
    [101, 100],
    [140, 100],
  ]) {
    assert.equal(miniOpacity(input), expected, String(input));
  }
});

test("mini opacity defaults invalid or non-finite inputs to opaque", () => {
  for (const input of [undefined, "junk", NaN, Infinity, -Infinity]) {
    assert.equal(miniOpacity(input), 100, String(input));
  }
});

test("mini opacity UI and saved preferences support the same 0–100 range", () => {
  assert.equal(OPACITY_MIN, 0);
  assert.equal(OPACITY_MAX, 100);
  assert.equal(OPACITY_STEP, 1);
  const rust = readFileSync("src-tauri/src/preferences.rs", "utf8");
  assert.match(
    rust,
    new RegExp(`"miniOpacity" => .*\\(${OPACITY_MIN}\\.\\.=${OPACITY_MAX}\\)`),
  );
  const slider = readFileSync(
    "src/lib/components/OpacitySlider.svelte",
    "utf8",
  );
  assert.match(slider, /export let min = OPACITY_MIN/);
  assert.match(
    slider,
    /<input[\s\S]*?type="range"[\s\S]*?\{min\}[\s\S]*?max=\{OPACITY_MAX\}[\s\S]*?step=\{OPACITY_STEP\}/,
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
