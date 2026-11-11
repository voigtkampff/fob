// Saved orders of process columns and summary cards. A saved list may come
// from an older or newer version: unknown and repeated ids are dropped and
// missing ones are put back at their default place, so every item shows once.

/** Dashboard tab cards (DASHBOARD_CARDS in src-tauri/src/preferences.rs). */
export const DASHBOARD_CARDS = [
  "cpu",
  "gpu",
  "memory",
  "disk",
  "network",
  "temp",
];
/** Processes tab stats row (PROCESS_CARDS in src-tauri/src/preferences.rs). */
export const PROCESS_CARDS = [
  "cpu",
  "memory",
  "gpu",
  "storage",
  "system",
  "network",
  "temp",
];

/**
 * @param {unknown} saved
 * @param {readonly string[]} defaults
 * @returns {string[]}
 */
export function normalizeOrder(saved, defaults) {
  const known = new Set(defaults);
  const order = [
    ...new Set(
      (Array.isArray(saved) ? saved : []).filter(
        (id) => typeof id === "string" && known.has(id),
      ),
    ),
  ];
  // A missing id goes right after the nearest default neighbour before it.
  defaults.forEach((id, i) => {
    if (order.includes(id)) return;
    const before = defaults
      .slice(0, i)
      .reverse()
      .find((d) => order.includes(d));
    order.splice(before ? order.indexOf(before) + 1 : 0, 0, id);
  });
  return order;
}

/** Moves id next to target (after it when after is true).
 * @param {string[]} order @param {string} id @param {string} target @param {boolean} after
 * @returns {string[]} */
export function moveNextTo(order, id, target, after) {
  if (id === target || !order.includes(id) || !order.includes(target))
    return order;
  const rest = order.filter((x) => x !== id);
  rest.splice(rest.indexOf(target) + (after ? 1 : 0), 0, id);
  return rest;
}

/** Moves id one step among the ids in shown (the visible ones), keeping
 * hidden ids where they are relative to the others.
 * @param {string[]} order @param {string} id @param {-1 | 1} step @param {readonly string[]} [shown]
 * @returns {string[]} */
export function stepOrder(order, id, step, shown = order) {
  const visible = order.filter((x) => shown.includes(x));
  const neighbour = visible[visible.indexOf(id) + step];
  return neighbour ? moveNextTo(order, id, neighbour, step > 0) : order;
}

/** The order a drag in progress would land in; the saved order without one.
 * @param {string[]} order @param {{ id: string, target: string, after: boolean } | null} move
 * @returns {string[]} */
export function previewOrder(order, move) {
  return move ? moveNextTo(order, move.id, move.target, move.after) : order;
}
