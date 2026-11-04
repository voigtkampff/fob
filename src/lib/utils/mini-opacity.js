// Fob: mini window opacity in percent, stepless 0–100 (preferences.rs,
// miniOpacity). Only the panel, card and popup backgrounds fade; at 0 the
// glass (blur, edge light, shadow) remains and text stays opaque.
export const OPACITY_MIN = 0;
export const OPACITY_MAX = 100;
export const OPACITY_STEP = 1;

/** A saved or typed value as a valid whole percent (default 100).
 * @param {unknown} value
 * @returns {number} */
export function miniOpacity(value) {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return OPACITY_MAX;
  return Math.min(OPACITY_MAX, Math.max(OPACITY_MIN, Math.round(n)));
}
