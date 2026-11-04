// Fob: mini window opacity in percent. The backend accepts 40–100
// (preferences.rs, miniOpacity); below 40 % text over a busy desktop is
// hard to read. Only the panel and card backgrounds fade; text stays opaque.
export const OPACITY_MIN = 40;
export const OPACITY_MAX = 100;
export const OPACITY_STEP = 5;

/** A saved or typed value as a valid whole percent (default 100).
 * @param {unknown} value
 * @returns {number} */
export function miniOpacity(value) {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return OPACITY_MAX;
  return Math.min(OPACITY_MAX, Math.max(OPACITY_MIN, Math.round(n)));
}
