/** @typedef {import("$lib/types").PhysicalDisk} PhysicalDisk */

/** Bus names that read the same in every language; others go through i18n
 * (`dh.bus.*`). */
export const BUS_NAMES = /** @type {Record<string, string>} */ ({
  nvme: "NVMe",
  sata: "SATA",
  ata: "ATA",
  atapi: "ATAPI",
  usb: "USB",
  raid: "RAID",
  scsi: "SCSI",
  sas: "SAS",
  sd: "SD",
  mmc: "MMC",
});

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

/** Temperature and life used as the drive reported them, from its NVMe
 * health log or its ATA statistics; null for anything not reported.
 * @param {PhysicalDisk} disk
 * @returns {{ temperature_c: number | null, percent_used: number | null }} */
export function headline(disk) {
  if (disk.nvme.kind === "ok")
    return {
      temperature_c: disk.nvme.log.temperature_c,
      percent_used: disk.nvme.log.percent_used,
    };
  if (disk.ata.kind === "ok")
    return {
      temperature_c: disk.ata.stats.temperature_c,
      percent_used: disk.ata.stats.percent_used,
    };
  return { temperature_c: null, percent_used: null };
}

/** The i18n key and value of the note under a disk: the query its bus is
 * asked (ATA statistics for SATA, the NVMe log for the rest), or none when
 * everything asked for was reported.
 * @param {PhysicalDisk} disk
 * @returns {{ key: string, vars?: Record<string, number> } | null} */
export function queryNote(disk) {
  if (disk.ata.kind === "ok") {
    const s = disk.ata.stats;
    return Object.values(s).some((v) => v == null)
      ? { key: "dh.ata.partial" }
      : null;
  }
  if (disk.ata.kind !== "not_asked")
    return disk.ata.kind === "error"
      ? { key: "dh.ata.error", vars: { v: disk.ata.code } }
      : { key: `dh.ata.${disk.ata.kind}` };
  if (disk.nvme.kind === "ok") return null;
  return disk.nvme.kind === "error"
    ? { key: "dh.nvme.error", vars: { v: disk.nvme.code } }
    : { key: `dh.nvme.${disk.nvme.kind}` };
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

export const REFRESH_MS = 5 * 60 * 1000;
export const PENDING_RETRY_MS = 10 * 1000;

/** @typedef {{ kind: "loading" } | { kind: "failed", message: string } | import("$lib/types").DiskHealthReport} DiskHealthView */

/** Asks for the disk health report now, every 5 minutes, and once more 10 s
 * after a pending reply, until the returned stop function runs. One request
 * at a time; the backend's 5 minute cache serves every open view.
 * @param {() => Promise<import("$lib/types").DiskHealthReport>} request
 * @param {(view: DiskHealthView, pending: boolean) => void} onChange
 * @returns {() => void} stop */
export function watchDiskHealth(request, onChange) {
  /** @type {DiskHealthView} */
  let view = { kind: "loading" };
  let live = true;
  let busy = false;
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let retry;
  const load = async () => {
    if (busy || !live) return;
    busy = true;
    try {
      const report = await request();
      if (!live) return;
      const pending = report.kind === "pending";
      if (!pending || view.kind === "loading") view = report;
      clearTimeout(retry);
      if (pending) retry = setTimeout(load, PENDING_RETRY_MS);
      onChange(view, pending);
    } catch (e) {
      if (live) {
        view = { kind: "failed", message: String(e) };
        onChange(view, false);
      }
    } finally {
      busy = false;
    }
  };
  onChange(view, false);
  void load();
  const timer = setInterval(load, REFRESH_MS);
  return () => {
    live = false;
    clearInterval(timer);
    clearTimeout(retry);
  };
}
