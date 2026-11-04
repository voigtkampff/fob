/** Whether a right-click on `target` may open the WebView's own menu:
 * only in editable text, where it offers copy and paste.
 * @param {EventTarget | null} target
 * @returns {boolean} */
export function allowsNativeMenu(target) {
  const el = /** @type {any} */ (target);
  if (!el || typeof el.closest !== "function") return false;
  const field = el.closest("input, textarea, [contenteditable]");
  if (!field) return false;
  if (field.isContentEditable) return true;
  if (field.tagName === "TEXTAREA") return !field.disabled;
  const type = (field.getAttribute("type") || "text").toLowerCase();
  return (
    ["text", "search", "url", "email", "tel", "password", "number"].includes(
      type,
    ) && !field.disabled
  );
}
