// Fob mini window: ONE fixed size for every view. The window is never resized
// on a view change or a data refresh, so its top edge never moves. Every block
// of the cards view has a fixed CSS height (below), so their sum is the window
// height and the cards view can never overflow, whatever the fonts or data.
// Detail views lay out inside the same height; only a long detail list
// (e.g. many fans) scrolls inside the window.
// src-tauri/src/companion.rs (MINI_HEIGHT) must match; a Rust test checks it.

/** Fixed heights (CSS px = logical px at any Windows scaling). */
export const MINI = {
  width: 360,
  padTop: 8,
  padBottom: 10,
  bar: 28, // title bar row
  barGap: 6,
  chips: 40, // two rows of 18 px chips + 4 px gap; a third row is cut, never wraps the layout
  chipRow: 18,
  gap: 8, // between header, card grid and the process list
  card: 118, // each of the six cards, border included
  cardRows: 3,
  cardGap: 8,
  procHead: 24,
  procRow: 27,
  procRows: 5, // always exactly this many rows, padded with empty ones
  procPad: 4, // top and bottom padding of the process card
  border: 1,
};

/** The top-processes card: header plus exactly `procRows` rows. */
export function miniProcsHeight(m = MINI) {
  return 2 * m.border + 2 * m.procPad + m.procHead + m.procRows * m.procRow;
}

/** Height of the cards view built from the fixed blocks above. */
export function miniCardsHeight(m = MINI) {
  const grid = m.cardRows * m.card + (m.cardRows - 1) * m.cardGap;
  const procs = miniProcsHeight(m);
  return (
    m.padTop +
    m.bar +
    m.barGap +
    m.chips +
    m.gap +
    grid +
    m.gap +
    procs +
    m.padBottom
  );
}

/** The window's logical height: the cards view plus a little slack for
 * DPI rounding of the client area. */
export const MINI_HEIGHT = 652;

/** CSS custom properties for the fixed blocks. */
export function miniStyle(m = MINI) {
  return Object.entries({ ...m, procs: miniProcsHeight(m) })
    .map(([k, v]) => `--mini-${k}:${v}px`)
    .join(";");
}

/** Exactly `count` rows: the list, then `null` placeholders.
 * @template T
 * @param {T[]} rows
 * @param {number} count
 * @returns {(T | null)[]}
 */
export function fixedRows(rows, count) {
  const list = (rows ?? []).slice(0, count);
  return [...list, ...Array(count - list.length).fill(null)];
}
