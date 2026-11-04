import assert from "node:assert/strict";
import { allowsNativeMenu } from "../src/lib/utils/context-menu.js";
import { miniOpacity, OPACITY_MIN, OPACITY_STEP } from "../src/lib/utils/mini-opacity.js";

const el = (tag, attrs = {}, extra = {}) => ({
  tagName: tag,
  disabled: false,
  isContentEditable: false,
  getAttribute: (k) => attrs[k] ?? null,
  closest(sel) { return sel.split(",").map((s) => s.trim().split("[")[0]).includes(tag.toLowerCase()) ? this : null; },
  ...extra,
});
assert.equal(allowsNativeMenu(el("DIV")), false, "plain page: no WebView menu");
assert.equal(allowsNativeMenu(null), false);
assert.equal(allowsNativeMenu(el("INPUT", { type: "text" })), true, "text input keeps copy/paste");
assert.equal(allowsNativeMenu(el("INPUT", { type: "range" })), false, "slider: no menu");
assert.equal(allowsNativeMenu(el("TEXTAREA")), true);
assert.equal(OPACITY_MIN, 0);
assert.equal(OPACITY_STEP, 1);
assert.equal(miniOpacity(0), 0, "0 % is allowed");
assert.equal(miniOpacity(37), 37, "stepless");
assert.equal(miniOpacity(-5), 0);
assert.equal(miniOpacity("x"), 100);
console.log("glass ok");
import { miniTint, inkBoost } from "../src/lib/utils/mini-opacity.js";
assert.equal(miniTint(undefined), 30, "frost defaults to 30");
assert.equal(miniTint(0), 0);
assert.equal(miniTint(140), 100);
assert.equal(inkBoost(100, 30), 0, "opaque window: theme text as is");
assert.ok(inkBoost(0, 0) === 1, "clear window, no frost: full contrast");
assert.ok(inkBoost(0, 100) < inkBoost(0, 0), "frost reduces the boost");
assert.ok(inkBoost(30, 30) > inkBoost(70, 30), "clearer window, stronger text");
console.log("frost/ink ok");
