export const MINI = {
  width: 360,
  padTop: 8,
  padBottom: 10,
  bar: 28,
  barGap: 6,
  chips: 40,
  chipRow: 18,
  gap: 8,
  card: 118,
  cardRows: 3,
  cardGap: 8,
  procHead: 24,
  procRow: 27,
  procRows: 5,
  procPad: 4,
  border: 1,
};

export function miniProcsHeight(m = MINI) {
  return 2 * m.border + 2 * m.procPad + m.procHead + m.procRows * m.procRow;
}

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

export function miniStyle(m = MINI) {
  return Object.entries({ ...m, procs: miniProcsHeight(m) })
    .map(([k, v]) => `--mini-${k}:${v}px`)
    .join(";");
}

/**
 * @template T
 * @param {T[]} rows
 * @param {number} count
 * @returns {(T | null)[]}
 */
export function fixedRows(rows, count) {
  const list = (rows ?? []).slice(0, count);
  return [...list, ...Array(count - list.length).fill(null)];
}
