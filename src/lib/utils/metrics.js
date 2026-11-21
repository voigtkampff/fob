/**
 * @param {number} used
 * @param {number} total
 * @returns {string}
 */
export function formatUsagePercentage(used, total) {
  if (total === 0) return "N/A";
  return `${((used / total) * 100).toFixed(1)}%`;
}
