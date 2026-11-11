/** Process table column widths.
 *
 * Every column starts at the width its own content needs: its widest zh or
 * en header with the sort arrow, and its longest usual value, with at least
 * 5 px to spare for font differences on Windows (fitWidths). Short columns
 * keep that width at any window size. The long text columns (process name,
 * command, root, environment) are elastic: they take spare room first, up to
 * a bound, and give it back first in a narrow window. Room left after that
 * stays empty before the actions column (the filler) instead of stretching
 * short values apart. Below the narrowest layout the region scrolls sideways.
 *
 * A width the user set by dragging a column's edge (behavior.columnWidths)
 * wins for that column; the others keep their own defaults. */

/** @type {Record<string, number>} */
const fitWidths = {
  pid: 72,
  ppid: 100,
  session_id: 100,
  status: 96,
  // An account name such as NETWORK SERVICE; longer names and unresolved
  // SIDs are cut and shown whole in the tooltip.
  user: 130,
  cpu_usage: 80,
  gpu_usage: 80,
  memory_usage: 96,
  gpu_memory: 135,
  virtual_memory: 96,
  disk_read: 100,
  disk_write: 100,
  run_time: 120,
  start_time: 160,
};
/** Tree view's family headers (进程族 CPU, Family RAM) are longer.
 * @type {Record<string, number>} */
const treeFitWidths = { cpu_usage: 105, memory_usage: 110 };
/** Elastic columns: [narrowest, widest]. They grow in this order.
 * @type {Record<string, [number, number]>} */
const elastic = {
  name: [180, 460],
  command: [200, 520],
  root: [200, 420],
  environ: [200, 420],
};
/** Tree view needs room for 10 levels of 16 px indent and the expander.
 * @type {[number, number]} */
const treeName = [280, 560];

/** Width of the sticky actions column (w-28). */
export const actionsWidth = 112;
/** Bounds for a width set by hand. */
export const manualMin = 48;
export const manualMax = 1200;

/** @param {string} id @param {boolean} tree @returns {[number, number] | null} */
function bounds(id, tree) {
  if (id === "name") return tree ? treeName : elastic.name;
  return elastic[id] ?? null;
}

/** The width a column starts from: its content width, or an elastic
 * column's narrowest.
 * @param {string} id @param {boolean} [tree] */
export function columnWidth(id, tree = false) {
  const range = bounds(id, tree);
  if (range) return range[0];
  return (
    (tree
      ? /** @type {Record<string, number>} */ (treeFitWidths)[id]
      : undefined) ??
    fitWidths[id] ??
    120
  );
}

/** Saved hand-set widths, kept only for known ids and finite numbers, rounded
 * and clamped. Anything else in the settings is ignored.
 * @param {unknown} saved @param {readonly string[]} ids
 * @returns {Record<string, number>} */
export function manualWidths(saved, ids) {
  if (!saved || typeof saved !== "object" || Array.isArray(saved)) return {};
  /** @type {Record<string, number>} */
  const kept = {};
  for (const [id, value] of Object.entries(saved))
    if (ids.includes(id) && typeof value === "number" && Number.isFinite(value))
      kept[id] = clampManual(value);
  return kept;
}

/** @param {number} width */
export const clampManual = (width) =>
  Math.round(Math.min(manualMax, Math.max(manualMin, width)));

/** Width of each visible column and of the empty filler before the actions
 * column, for a table region of the given width.
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} tree
 * @param {number} region @param {Record<string, number>} [manual]
 * @returns {{ widths: Record<string, number>, filler: number }} */
export function columnLayout(columns, tree, region, manual = {}) {
  const shown = columns.filter((c) => c.visible).map((c) => c.id);
  /** @type {Record<string, number>} */
  const widths = Object.fromEntries(
    shown.map((id) => [id, manual[id] ?? columnWidth(id, tree)]),
  );
  let spare = Math.floor(region - tableWidth(columns, tree, manual));
  for (const id of [
    "name",
    ...Object.keys(elastic).filter((id) => id !== "name"),
  ]) {
    const range = bounds(id, tree);
    if (spare <= 0 || !range || !(id in widths) || id in manual) continue;
    const grow = Math.min(range[1] - range[0], spare);
    widths[id] += grow;
    spare -= grow;
  }
  return { widths, filler: Math.max(0, spare) };
}

/** Narrowest the table gets before the region scrolls sideways.
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} [tree]
 * @param {Record<string, number>} [manual] */
export function tableWidth(columns, tree = false, manual = {}) {
  return columns
    .filter((c) => c.visible)
    .reduce(
      (sum, c) => sum + (manual[c.id] ?? columnWidth(c.id, tree)),
      actionsWidth,
    );
}
