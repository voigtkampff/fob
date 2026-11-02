// Fob: how a CPU card / detail view shows its logical processors, chosen
// by thread count so the grid always fits its fixed-height area (no scroll).

/** A card's core area: the 7-card row at a 1024 px window leaves ~112 px
 * of content width; the middle band is 56 px tall. */
export const CARD_AREA = { width: 108, height: 56 };

/**
 * @param {number} threads
 * @param {"card" | "detail"} where
 * @returns {{ mode: "cells", cols: number, rows: number, labels: true }
 *   | { mode: "heatmap", cols: number, rows: number, labels: false }
 *   | { mode: "summary", top: number }}
 *  cells: a labeled bar per thread; heatmap: unlabeled squares whose colour
 *  is the load (hover shows "CPU n · x%"); summary: average + busiest threads
 *  (the full grid is in the detail view).
 */
export function coreLayout(threads, where = "card") {
  const n = Math.max(0, threads | 0);
  if (where === "detail") {
    // Detail views have room for 16-24 labeled bars per row; above 32
    // threads every thread is still shown, as heat cells.
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

/** Row height (px) of each mode inside a card; used by the layout and the fit test. */
export const CARD_ROW = { cells: 17, heatmap: 11 };
/** Smallest cell width (px) that still reads (2-digit label / a visible square). */
export const CARD_MIN_CELL = { cells: 11, heatmap: 5 };
/** Gap (px) between cells. */
export const CARD_GAP = { cells: 2, heatmap: 1 };

/** The busiest `count` threads, busiest first. @param {number[]} usage */
export function busiestThreads(usage, count = 3) {
  return usage
    .map((value, index) => ({ index, value }))
    .filter((c) => Number.isFinite(c.value))
    .sort((a, b) => b.value - a.value || a.index - b.index)
    .slice(0, count);
}
