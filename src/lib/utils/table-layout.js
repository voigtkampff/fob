/** Stable column widths keep long optional fields readable through
 * deliberate horizontal scrolling.
 * Number columns take their width from the kind of value they show, so
 * columns of one kind line up. Each width fits the widest zh or en header
 * of its kind, sort arrow included, and the longest value, with at least
 * 5 px to spare for font differences on Windows. */
const kindWidths = {
  id: 100,
  percent: 105,
  rate: 95,
  bytes: 135,
  duration: 130,
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
/** @param {string} id @param {boolean} tree */
export function columnWidth(id, tree = false) {
  if (id === "name" && tree) return 340;
  const kind = columnKinds[id];
  return kind ? kindWidths[kind] : (textWidths[id] ?? 220);
}
/** @param {{id:string,visible:boolean}[]} columns @param {boolean} tree */
export function tableWidth(columns, tree = false) {
  return (
    112 +
    columns
      .filter((c) => c.visible)
      .reduce((sum, c) => sum + columnWidth(c.id, tree), 0)
  );
}
