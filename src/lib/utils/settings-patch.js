// Send only changed leaves: one window must not overwrite another window's preferences.
// A map keyed by column id is one value: a key the user removed (a column
// width reset to automatic) has to go away, which a leaf patch cannot say.
// The native side replaces it whole too (preferences.rs `merge`).
const WHOLE = new Set(["columnWidths"]);
/** @param {string} key @param {any} value */
const branch = (key, value) =>
  !!value &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  !WHOLE.has(key);
/** @param {any} previous @param {any} next @returns {Record<string, any>} */
export function changedLeaves(previous, next) {
  /** @type {Record<string, any>} */
  const patch = {};
  for (const [key, value] of Object.entries(next)) {
    if (WHOLE.has(key)) {
      if (JSON.stringify(previous?.[key] ?? null) !== JSON.stringify(value))
        patch[key] = value;
    } else if (branch(key, value)) {
      const nested = changedLeaves(previous?.[key] || {}, value);
      if (Object.keys(nested).length) patch[key] = nested;
    } else if (previous?.[key] !== value) patch[key] = value;
  }
  return patch;
}
/** @param {any} base @param {any} patch @returns {any} */
export function mergePreferences(base, patch) {
  const next = { ...base };
  for (const [key, value] of Object.entries(patch)) {
    next[key] = branch(key, value)
      ? mergePreferences(base[key] || {}, value)
      : value;
  }
  return next;
}
