// Fob 0.7.12: one look for every history chart. The Processes page cards
// (stats/ResourceTrend.svelte) are the source; the dashboard, its detail
// view and the mini window (Dashboard compact) draw through Spark.svelte.

/** Line colour per sample key, from the theme's chart tokens. */
/** @type {Record<string, string>} */
export const SERIES_COLOR = {
  cpu: "var(--chart-1)",
  gpu: "var(--chart-2)",
  memory: "var(--chart-3)",
  cpu_temp: "var(--chart-1)",
  gpu_temp: "var(--chart-2)",
  received: "var(--chart-1)",
  sent: "var(--chart-4)",
  disk_read: "var(--chart-4)",
  disk_write: "var(--chart-2)",
};
/** The outgoing line of a pair is dashed. */
export const DASHED = new Set(["sent", "disk_write"]);
export const STROKE_WIDTH = 1.5;
/** Fill under the line: the colour fading from this opacity at the top. */
export const FILL_TOP = 0.32;
export const FILL_BOTTOM = 0.01;

/** Close each run of a line path ("M…L…") down to the baseline.
 * @param {string} line
 * @param {number} height
 */
export function sparkAreaPath(line, height) {
  return line
    .split("M")
    .filter(Boolean)
    .map((/** @type {string} */ run) => {
      const points = run.split("L");
      const first = points[0].split(",")[0];
      const last = points[points.length - 1].split(",")[0];
      return `M${run}L${last},${height}L${first},${height}Z`;
    })
    .join("");
}
