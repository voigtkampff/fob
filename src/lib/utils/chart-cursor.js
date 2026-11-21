import { SPARK_WINDOW_MS } from "./dashboard.js";

/**
 * @template {{ timestamp: number }} T
 * @param {T[]} history
 * @param {number} ratio
 * @param {{ end?: number, windowMs?: number }} [axis]
 * @returns {{ sample: T, x: number } | null} x: the sample's position, 0..1
 */
export function cursorAt(
  history,
  ratio,
  { end, windowMs = SPARK_WINDOW_MS } = {},
) {
  const list = history ?? [];
  if (!list.length || windowMs <= 0) return null;
  const right = end ?? list[list.length - 1].timestamp;
  const left = right - windowMs;
  const at = left + Math.min(1, Math.max(0, ratio)) * windowMs;
  let best = null;
  for (const sample of list) {
    if (sample.timestamp < left || sample.timestamp > right) continue;
    if (
      !best ||
      Math.abs(sample.timestamp - at) < Math.abs(best.timestamp - at)
    )
      best = sample;
  }
  return best ? { sample: best, x: (best.timestamp - left) / windowMs } : null;
}

/**
 * @param {number} x @param {number} tip @param {number} box @param {number} [gap]
 */
export function tooltipLeft(x, tip, box, gap = 6) {
  const right = x + gap;
  if (right + tip <= box) return right;
  const left = x - gap - tip;
  return Math.max(0, Math.min(left, box - tip));
}

/** @param {number} stamp */
export function clockTime(stamp) {
  const d = new Date(stamp);
  return [d.getHours(), d.getMinutes(), d.getSeconds()]
    .map((n) => String(n).padStart(2, "0"))
    .join(":");
}

/** @param {number} v */
export const percentText = (v) => `${v.toFixed(1)} %`;
