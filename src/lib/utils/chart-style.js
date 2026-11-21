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
export const DASHED = new Set(["sent", "disk_write"]);
export const STROKE_WIDTH = 1.5;
export const FILL_TOP = 0.32;
export const FILL_BOTTOM = 0.01;

/**
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
