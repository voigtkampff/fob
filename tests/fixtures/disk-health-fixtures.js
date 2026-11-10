// Generic disk health replies for the preview. Not read from any real disk.
const TB = 1e12;
/** @typedef {import("../../src/lib/types").PhysicalDisk} Disk */
/** @param {Partial<import("../../src/lib/types").NvmeHealthLog>} over @returns {import("../../src/lib/types").NvmeHealthLog} */
const log = (over = {}) => ({
  critical_warning: {
    spare_low: false,
    temperature: false,
    reliability: false,
    read_only: false,
    backup_failed: false,
    pmr_read_only: false,
    raw: 0,
  },
  temperature_c: 41,
  available_spare: 100,
  spare_threshold: 10,
  percent_used: 3,
  data_read_bytes: 48.2 * TB,
  data_written_bytes: 36.9 * TB,
  power_cycles: 1284,
  power_on_hours: 9876,
  unsafe_shutdowns: 41,
  media_errors: 0,
  error_log_entries: 12,
  warning_temp_minutes: 0,
  critical_temp_minutes: 0,
  ...over,
});
/** @type {import("../../src/lib/types").WindowsDiskStatus} */
const healthy = {
  kind: "ok",
  health: "healthy",
  operational: [{ kind: "ok" }],
};
/** @param {number} number @param {string[]} volumes @param {Partial<import("../../src/lib/types").NvmeHealthLog>} [over] @returns {Disk} */
const nvme = (number, volumes, over = {}) => ({
  number,
  model: "NVMe SSD 2TB",
  bus: "nvme",
  removable: false,
  volumes,
  windows: healthy,
  nvme: { kind: "ok", log: log(over) },
});
const checked_at_ms = Date.UTC(2026, 0, 1, 9, 30);
/** @type {Record<string, import("../../src/lib/types").DiskHealthReport>} */
export const REPLIES = {
  ok: {
    kind: "ok",
    checked_at_ms,
    disks: [
      nvme(0, ["C:\\", "D:\\"]),
      {
        number: 1,
        model: "SATA SSD 1TB",
        bus: "sata",
        removable: false,
        volumes: ["E:\\"],
        windows: healthy,
        nvme: { kind: "not_asked" },
      },
      {
        number: 3,
        model: "USB Disk",
        bus: "usb",
        removable: true,
        volumes: [],
        windows: { kind: "not_reported" },
        nvme: { kind: "unsupported" },
      },
    ],
  },
  warning: {
    kind: "ok",
    checked_at_ms,
    disks: [
      nvme(0, ["C:\\"], {
        critical_warning: {
          spare_low: true,
          temperature: true,
          reliability: false,
          read_only: false,
          backup_failed: false,
          pmr_read_only: false,
          raw: 3,
        },
        temperature_c: 78,
        available_spare: 4,
        percent_used: 104,
        media_errors: 3,
        warning_temp_minutes: 125,
      }),
      {
        number: 1,
        model: "SATA HDD 4TB",
        bus: "sata",
        removable: false,
        volumes: ["F:\\"],
        windows: {
          kind: "ok",
          health: "warning",
          operational: [{ kind: "ok" }, { kind: "predictive_failure" }],
        },
        nvme: { kind: "not_asked" },
      },
    ],
  },
  errors: {
    kind: "ok",
    checked_at_ms,
    disks: [
      {
        ...nvme(0, ["C:\\"]),
        nvme: { kind: "denied" },
        windows: { kind: "failed", code: -2147217405 },
      },
      {
        number: 2,
        model: "RAID Volume",
        bus: "raid",
        removable: false,
        volumes: ["G:\\"],
        windows: { kind: "not_reported" },
        nvme: { kind: "error", code: 1117 },
      },
      {
        number: null,
        model: null,
        bus: "unknown",
        removable: false,
        volumes: [],
        windows: { kind: "not_reported" },
        nvme: { kind: "bad_reply" },
      },
    ],
  },
  empty: { kind: "ok", checked_at_ms, disks: [] },
  unavailable: { kind: "unavailable" },
  pending: { kind: "pending" },
};
