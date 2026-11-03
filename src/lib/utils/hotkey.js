// Fob: the mini window hotkey recorder: turns a keydown into "Ctrl+Alt+F"
// and checks it with the same rules as src-tauri/src/hotkey.rs (which has
// the last word when it registers the combo).

/** KeyboardEvent.code → the key name Fob saves.
 * @type {Record<string, string>} */
const NAMED = {
  Space: "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Insert",
  Delete: "Delete",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backslash: "\\",
};
const MODIFIER_CODES = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

/** @param {string} code KeyboardEvent.code */
export function keyName(code) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  return NAMED[code] ?? null;
}

/** @param {{ ctrl?: boolean, alt?: boolean, shift?: boolean, win?: boolean }} m */
function modifiers(m) {
  return [
    m.ctrl && "Ctrl",
    m.alt && "Alt",
    m.shift && "Shift",
    m.win && "Win",
  ].filter(Boolean);
}

/** A keydown while recording.
 * @param {{ code: string, ctrlKey: boolean, altKey: boolean, shiftKey: boolean, metaKey: boolean }} e
 * @returns {{ combo: string | null, held: string }} combo: the full combo
 *   once a non-modifier key is pressed (null while only modifiers are held,
 *   or for a key Fob can't use); held: what to show meanwhile ("Ctrl+Alt+").
 */
export function fromKeydown(e) {
  const mods = modifiers({
    ctrl: e.ctrlKey,
    alt: e.altKey,
    shift: e.shiftKey,
    win: e.metaKey,
  });
  const held = mods.length ? `${mods.join("+")}+` : "";
  if (MODIFIER_CODES.test(e.code)) return { combo: null, held };
  const key = keyName(e.code);
  return { combo: key ? [...mods, key].join("+") : null, held };
}

/** null when `combo` is acceptable, else the reason ("invalid" |
 * "needsModifier"), as hotkey.rs `parse` decides.
 * @param {string} combo
 */
export function validate(combo) {
  const parts = combo.split("+");
  const key = parts.at(-1) ?? "";
  const mods = parts.slice(0, -1);
  const known = Object.values(NAMED);
  const keyOk =
    /^[A-Z0-9]$/.test(key) ||
    /^F([1-9]|1[0-9]|2[0-4])$/.test(key) ||
    known.includes(key);
  if (parts.some((p) => p === "") && !(parts.length === 1 && keyOk))
    return "invalid";
  if (!keyOk) return "invalid";
  const set = new Set(mods);
  if (
    set.size !== mods.length ||
    mods.some((m) => !["Ctrl", "Alt", "Shift", "Win"].includes(m))
  )
    return "invalid";
  const fkey = /^F\d/.test(key);
  if (
    !(
      set.has("Ctrl") ||
      set.has("Alt") ||
      set.has("Win") ||
      (set.has("Shift") && fkey)
    )
  )
    return "needsModifier";
  return null;
}
