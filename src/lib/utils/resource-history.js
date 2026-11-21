export const RESOURCE_WINDOW_MS = 120_000;
/**
 * @param {import('../types').ResourceSample[]} history
 * @param {("received" | "sent" | "disk_read" | "disk_write")[]} [keys]
 */
export function networkCeiling(history, keys = ["received", "sent"]) {
  const peak = Math.max(
    1024,
    ...history.flatMap((p) => keys.map((k) => p[k] ?? 0)),
  );
  const magnitude = 10 ** Math.floor(Math.log10(peak));
  return Math.ceil(peak / magnitude) * magnitude;
}
/**
 * @param {import('../types').ResourceSample[]} history
 * @param {number} timestamp
 */
export function nearestResourceSample(history, timestamp) {
  return history.reduce(
    (best, p) =>
      !best ||
      Math.abs(p.timestamp - timestamp) < Math.abs(best.timestamp - timestamp)
        ? p
        : best,
    /** @type {import('../types').ResourceSample|undefined} */ (undefined),
  );
}
