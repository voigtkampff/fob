/**
 * Apply the native retention floor, not the browser clock: delayed and hidden
 * windows keep exactly the same samples as the independent overview consumers.
 * @param {import('../types').ResourceSample[]} current
 * @param {import('../types').HistoryUpdate} update
 */
export function mergeHistory(current, update) {
  if (update.reset) return update.samples;
  const first = update.first_timestamp;
  if (first == null) return [];
  return current.filter((s) => s.timestamp >= first).concat(update.samples);
}
