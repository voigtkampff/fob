/** Stable column widths keep long optional fields readable through
 * deliberate horizontal scrolling.
 * Widths for a given window come from columnWidths: the name column grows
 * to nameMaxWidth, then the other columns share the rest equally. The name
 * column shrinks to nameMinWidth before the table scrolls sideways. A table
 * a little wider than the window would otherwise hide the end of the last
 * column under the sticky actions column, which reads as values running
 * into the buttons.
 * columnWidth is each column's own minimum, from the kind of value it
 * shows; the shared data column width is the widest of these. Each fits the
 * widest zh or en header of its kind, sort arrow included, and the longest
 * value, with at least 5 px to spare for font differences on Windows. */
const kindWidths = {
  id: 100,
  percent: 105,
  rate: 95,
  bytes: 135,
  duration: 120,
};
/** @type {Record<string, keyof typeof kindWidths>} */
export const columnKinds = {
  pid: "id",
  ppid: "id",
  session_id: "id",
  cpu_usage: "percent",
  gpu_usage: "percent",
  disk_read: "rate",
  disk_write: "rate",
  memory_usage: "bytes",
  gpu_memory: "bytes",
  virtual_memory: "bytes",
  run_time: "duration",
};
/** @type {Record<string, number>} */
const textWidths = { name: 250, status: 100, user: 65, start_time: 180 };
/** Narrowest the name column gets, at 10 levels of tree indent in tree view. */
export const nameMinWidth = (tree = false) => (tree ? 280 : 200);
/** Widest the name column gets: everything its cell can show. ProcessRow
 * cuts the name at 256 px (max-w-64); around it are the cell's 16 px left
 * and 8 px right padding, the 16 px icon, the pin and 8 px gaps. Tree view
 * adds 10 levels of 16 px indent, the 24 px expander and the family count. */
export const nameMaxWidth = (tree = false) => (tree ? 560 : 330);
/** Width of each visible column for a table region of the given width.
 * Every column but the name column gets the same width, at least the widest
 * of their own minimums (columnWidth), so headers and values fit in all of
 * them. The name column grows first, up to nameMaxWidth; the rest is shared
 * equally (within 1 px of rounding) so the table fills the region. Below the
 * minimum width nothing shrinks further and the region scrolls sideways.
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} tree @param {number} region
 * @returns {Record<string, number>} */
export function columnWidths(columns, tree, region) {
  const others = columns.filter((c) => c.visible && c.id !== "name");
  const spare = Math.max(0, Math.floor(region - tableWidth(columns, tree)));
  const grow = others.length
    ? Math.min(nameMaxWidth(tree) - nameMinWidth(tree), spare)
    : spare;
  const total = others.length * dataWidth(columns, tree) + spare - grow;
  const each = others.length ? Math.floor(total / others.length) : 0;
  const extra = total - each * others.length;
  return Object.fromEntries([
    ["name", nameMinWidth(tree) + grow],
    ...others.map((c, i) => [c.id, each + (i < extra ? 1 : 0)]),
  ]);
}
/** The one width every visible column but the name column starts from.
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} tree */
export function dataWidth(columns, tree = false) {
  return Math.max(
    0,
    ...columns
      .filter((c) => c.visible && c.id !== "name")
      .map((c) => columnWidth(c.id, tree)),
  );
}
/** Width of the sticky actions column (w-28). */
export const actionsWidth = 112;
/** @param {string} id @param {boolean} tree */
export function columnWidth(id, tree = false) {
  if (id === "name") return nameMinWidth(tree);
  const kind = columnKinds[id];
  return kind ? kindWidths[kind] : (textWidths[id] ?? 220);
}
/** Narrowest the table gets before it scrolls sideways.
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} tree */
export function tableWidth(columns, tree = false) {
  const others = columns.filter((c) => c.visible && c.id !== "name").length;
  return actionsWidth + nameMinWidth(tree) + others * dataWidth(columns, tree);
}
