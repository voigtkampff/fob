import assert from "node:assert/strict";
import test from "node:test";
import { fromKeydown, keyName, validate } from "../src/lib/utils/hotkey.js";

const down = (code, mods = {}) => ({
  code,
  ctrlKey: !!mods.ctrl,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift,
  metaKey: !!mods.win,
});

test("key codes map to saved names", () => {
  assert.equal(keyName("KeyF"), "F");
  assert.equal(keyName("Digit7"), "7");
  assert.equal(keyName("F24"), "F24");
  assert.equal(keyName("F25"), null);
  assert.equal(keyName("ArrowUp"), "Up");
  assert.equal(keyName("Backquote"), "`");
  assert.equal(keyName("NumpadEnter"), null);
});

test("a keydown records the combo in canonical order", () => {
  assert.deepEqual(fromKeydown(down("KeyF", { alt: true, ctrl: true })), {
    combo: "Ctrl+Alt+F",
    held: "Ctrl+Alt+",
  });
  assert.equal(
    fromKeydown(down("Space", { win: true, shift: true, ctrl: true })).combo,
    "Ctrl+Shift+Win+Space",
  );
  // Only modifiers so far: keep recording and show them.
  assert.deepEqual(fromKeydown(down("ControlLeft", { ctrl: true })), {
    combo: null,
    held: "Ctrl+",
  });
  assert.equal(fromKeydown(down("NumpadEnter", { ctrl: true })).combo, null);
});

test("validation matches the Rust rules", () => {
  for (const ok of ["Ctrl+Alt+F", "Win+1", "Shift+F5", "Ctrl+`", "Alt+PageUp"])
    assert.equal(validate(ok), null, ok);
  for (const text of ["F", "F5", "Shift+F", "Space"])
    assert.equal(validate(text), "needsModifier", text);
  for (const text of [
    "",
    "Ctrl+",
    "Ctrl+Alt",
    "Ctrl+Ctrl+F",
    "Hyper+F",
    "Ctrl+F25",
    "Ctrl+AB",
  ])
    assert.equal(validate(text), "invalid", JSON.stringify(text));
});
