export const OPACITY_MIN = 0;
export const OPACITY_MAX = 100;
export const OPACITY_STEP = 1;

/**
 * @param {unknown} value
 * @returns {number} */
export function miniOpacity(value) {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return OPACITY_MAX;
  return Math.min(OPACITY_MAX, Math.max(OPACITY_MIN, Math.round(n)));
}
