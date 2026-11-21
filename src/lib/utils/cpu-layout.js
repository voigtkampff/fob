export const CARD_AREA = { width: 108, height: 56 };

/**
 * @param {number} threads
 * @param {"card" | "detail"} where
 * @returns {{ mode: "cells", cols: number, rows: number, labels: true }
 *   | { mode: "heatmap", cols: number, rows: number, labels: false }
 *   | { mode: "summary", top: number }}
 */
export function coreLayout(threads, where = "card") {
  const n = Math.max(0, threads | 0);
  if (where === "detail") {
    if (n <= 32)
      return {
        mode: "cells",
        cols: Math.min(n, 16) || 1,
        rows: Math.ceil(n / 16),
        labels: true,
      };
    const cols = 32;
    return { mode: "heatmap", cols, rows: Math.ceil(n / cols), labels: false };
  }
  if (n <= 24) {
    const cols = Math.min(Math.max(n, 1), 8);
    return { mode: "cells", cols, rows: Math.ceil(n / cols), labels: true };
  }
  if (n <= 64)
    return {
      mode: "heatmap",
      cols: 16,
      rows: Math.ceil(n / 16),
      labels: false,
    };
  return { mode: "summary", top: 3 };
}

export const CARD_ROW = { cells: 17, heatmap: 11 };
export const CARD_MIN_CELL = { cells: 11, heatmap: 5 };
export const CARD_GAP = { cells: 2, heatmap: 1 };

/** @param {number[]} usage */
export function busiestThreads(usage, count = 3) {
  return usage
    .map((value, index) => ({ index, value }))
    .filter((c) => Number.isFinite(c.value))
    .sort((a, b) => b.value - a.value || a.index - b.index)
    .slice(0, count);
}
