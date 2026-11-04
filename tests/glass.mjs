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
