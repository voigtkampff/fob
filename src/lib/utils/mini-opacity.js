// Fob: mini window opacity in percent, stepless 0–100 (preferences.rs,
// miniOpacity). Only the panel, card and popup backgrounds fade; at 0 the
// glass (blur, edge light, shadow) remains and text stays opaque.
export const OPACITY_MIN = 0;
export const OPACITY_MAX = 100;
export const OPACITY_STEP = 1;

export const TINT_DEFAULT = 30;

/** A saved frost (tint strength) as a whole percent 0–100 (default 30).
 * @param {unknown} value
 * @returns {number} */
export function miniTint(value) {
  const n = typeof value === "number" ? value : Number(value);
  if (value === undefined || value === null || !Number.isFinite(n))
    return TINT_DEFAULT;
  return Math.min(100, Math.max(0, Math.round(n)));
}

/** How far text is pushed toward full contrast (0–1): more as the window
 * gets clearer, less when the frost already covers the desktop.
 * @param {number} opacity 0–100
 * @param {number} tint 0–100
 * @returns {number} */
export function inkBoost(opacity, tint) {
  return (1 - opacity / 100) * (1 - (tint / 100) * 0.5);
}

/** A saved or typed value as a valid whole percent (default 100).
 * @param {unknown} value
 * @returns {number} */
export function miniOpacity(value) {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return OPACITY_MAX;
  return Math.min(OPACITY_MAX, Math.max(OPACITY_MIN, Math.round(n)));
}
