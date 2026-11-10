/** @typedef {import("$lib/types").PhysicalDisk} PhysicalDisk */

/** NVMe counts data in decimal units, so sizes here are decimal too.
 * @param {number} bytes */
export function formatDecimalBytes(bytes) {
  const units = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
  let value = bytes;
  let i = 0;
  while (value >= 1000 && i < units.length - 1) {
    value /= 1000;
    i++;
  }
  return `${value >= 100 || i === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[i]}`;
}

/** Counters are 128-bit on the drive; past 10^15 show the magnitude.
 * @param {number} n */
export function formatCount(n) {
  return n < 1e15 ? Math.round(n).toLocaleString("en-US") : n.toExponential(2);
}

const WARNINGS = /** @type {const} */ ([
  "spare_low",
  "temperature",
  "reliability",
  "read_only",
  "backup_failed",
  "pmr_read_only",
]);

/** The drive's own Critical Warning flags that are set.
 * @param {PhysicalDisk} disk
 * @returns {(typeof WARNINGS)[number][]} */
export function driveWarnings(disk) {
  if (disk.nvme.kind !== "ok") return [];
  const w = disk.nvme.log.critical_warning;
  return WARNINGS.filter((k) => w[k]);
}

/** Operational codes Windows names as a problem. Unknown codes are shown
 * as their number, not as a problem.
 * @param {import("$lib/types").DiskOperational} op */
export const operationalBad = (op) => op.kind !== "ok" && op.kind !== "other";

/** True only when the drive or Windows reported a problem.
 * @param {PhysicalDisk} disk */
export function diskAlert(disk) {
  if (driveWarnings(disk).length) return true;
  if (disk.windows.kind !== "ok") return false;
  return (
    disk.windows.health === "warning" ||
    disk.windows.health === "unhealthy" ||
    disk.windows.operational.some(operationalBad)
  );
}
