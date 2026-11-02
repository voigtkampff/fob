// winperf mini window sizing: the window height is measured once per view
// (loading, the cards, or one detail view) and kept while data refreshes, so
// lists that change length every second scroll inside instead of moving the
// window's top edge.

/** The view the mini window is currently sized for.
 * @param {boolean} hasData
 * @param {string | null} detail
 */
export function miniView(hasData, detail) {
  return hasData ? (detail ?? "cards") : "loading";
}

/** Resize only when the view changed since the last fit.
 * @param {string | null} fitted view the window was last sized for (null: never / reopened)
 * @param {string} view
 */
export function needsFit(fitted, view) {
  return fitted !== view;
}
