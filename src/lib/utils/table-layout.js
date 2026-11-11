/** Stable column widths keep long optional fields readable through
 * deliberate horizontal scrolling.
 * The name column takes what the window leaves, between nameMinWidth and
 * nameMaxWidth, and shrinks to nameMinWidth before the table scrolls
 * sideways. A table a
 * little wider than the window would otherwise hide the end of the last
 * column under the sticky actions column, which reads as values running
 * into the buttons.
 * Number columns take their width from the kind of value they show, so
 * columns of one kind line up. Each width fits the widest zh or en header
 * of its kind, sort arrow included, and the longest value, with at least
 * 5 px to spare for font differences on Windows. */
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
 * adds 10 levels of 16 px indent, the 24 px expander and the family count.
 * More room would only be blank space before the next column; it goes to
 * the spacer after the actions column instead. */
export const nameMaxWidth = (tree = false) => (tree ? 560 : 330);
/** Name column width for a table region of the given width.
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} tree @param {number} region */
export function nameWidth(columns, tree, region) {
  const others = tableWidth(columns, tree) - nameMinWidth(tree);
  return Math.max(
    nameMinWidth(tree),
    Math.min(nameMaxWidth(tree), Math.floor(region - others)),
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
  return (
    actionsWidth +
    columns
      .filter((c) => c.visible)
      .reduce((sum, c) => sum + columnWidth(c.id, tree), 0)
  );
}
