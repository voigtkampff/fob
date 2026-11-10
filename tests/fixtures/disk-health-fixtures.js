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
  ata: { kind: "not_asked" },
});
/** @type {import("../../src/lib/types").AtaStats} */
const noStats = {
  temperature_c: null,
  percent_used: null,
  power_on_hours: null,
  power_cycles: null,
  data_read_bytes: null,
  data_written_bytes: null,
};
/** @param {number} number @param {string} model @param {string[]} volumes @param {import("../../src/lib/types").AtaState} ata @returns {Disk} */
const sata = (number, model, volumes, ata) => ({
  number,
  model,
  bus: "sata",
  removable: false,
  volumes,
  windows: healthy,
  nvme: { kind: "not_asked" },
  ata,
});
const checked_at_ms = Date.UTC(2026, 0, 1, 9, 30);
/** @type {Record<string, import("../../src/lib/types").DiskHealthReport>} */
export const REPLIES = {
  ok: {
    kind: "ok",
    checked_at_ms,
    disks: [
      nvme(0, ["C:\\", "D:\\"]),
      sata(1, "SATA SSD 1TB", ["E:\\"], {
        kind: "ok",
        stats: {
          temperature_c: 36,
          percent_used: 7,
          power_on_hours: 15234,
          power_cycles: 2210,
          data_read_bytes: 22.1 * TB,
          data_written_bytes: 18.4 * TB,
        },
      }),
      sata(2, "SATA HDD 4TB", ["F:\\"], {
        kind: "ok",
        stats: {
          ...noStats,
          temperature_c: 33,
          power_on_hours: 30411,
          power_cycles: 978,
        },
      }),
      {
        number: 3,
        model: "USB Disk",
        bus: "usb",
        removable: true,
        volumes: [],
        windows: { kind: "not_reported" },
        nvme: { kind: "unsupported" },
        ata: { kind: "not_asked" },
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
        ata: { kind: "sleeping" },
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
        ata: { kind: "not_asked" },
      },
      sata(4, "SATA SSD 500GB", ["K:\\"], { kind: "denied" }),
      sata(5, "SATA HDD 2TB", ["L:\\"], { kind: "unsupported" }),
      sata(6, "SATA SSD 256GB", [], { kind: "bad_reply" }),
      sata(7, "SATA HDD 1TB", [], { kind: "error", code: 1117 }),
      {
        number: null,
        model: null,
        bus: "unknown",
        removable: false,
        volumes: [],
        windows: { kind: "not_reported" },
        nvme: { kind: "bad_reply" },
        ata: { kind: "not_asked" },
      },
    ],
  },
  long: {
    kind: "ok",
    checked_at_ms,
    disks: [
      {
        ...nvme(10, ["C:\\", "D:\\", "E:\\", "F:\\", "G:\\", "H:\\", "I:\\"]),
        model:
          "Example Very Long Enterprise NVMe Solid State Drive Model 7680GB",
      },
      {
        number: 11,
        model: "Example External Enclosure With A Long Product Name",
        bus: "usb",
        removable: true,
        volumes: [],
        windows: { kind: "failed", code: -2147217405 },
        nvme: { kind: "unsupported" },
        ata: { kind: "not_asked" },
      },
      {
        number: null,
        model: null,
        bus: "spaces",
        removable: false,
        volumes: ["J:\\"],
        windows: {
          kind: "ok",
          health: "unhealthy",
          operational: [{ kind: "non_recoverable" }],
        },
        nvme: { kind: "not_asked" },
        ata: { kind: "not_asked" },
      },
    ],
  },
  empty: { kind: "ok", checked_at_ms, disks: [] },
  unavailable: { kind: "unavailable" },
  pending: { kind: "pending" },
};

// The capacity side the Disk detail and storage panel show next to health.
const GB = 1024 ** 3;
/** @param {string} mount @param {string} name @param {number} total @param {number} available */
const volume = (mount, name, total, available) => ({
  name,
  mount,
  file_system: "NTFS",
  total: total * GB,
  available: available * GB,
  read_only: false,
  read: 0,
  written: 0,
});
/** @type {import("../../src/lib/types").SystemStats} */
export const stats = {
  details: {
    identity: {
      cpu_model: "Example CPU",
      physical_cores: 8,
      architecture: "x86_64",
      platform: "windows",
      os_version: "Windows 11",
      kernel_version: "26100",
    },
    memory_unallocated: 0,
    swap_total: 0,
    swap_used: 0,
    volumes: [
      volume("C:\\", "Windows", 1000, 420),
      volume("D:\\", "Data", 860, 610),
      volume("E:\\", "Games", 930, 200),
    ],
    interfaces: [],
  },
  cpu_usage: [10, 20],
  memory_total: 32 * GB,
  memory_used: 14 * GB,
  memory_free: 18 * GB,
  memory_cached: 0,
  uptime: 3600,
  load_avg: [0, 0, 0],
  network_rx_bytes: 0,
  network_tx_bytes: 0,
  disk_total_bytes: 2790 * GB,
  disk_used_bytes: 1560 * GB,
  disk_free_bytes: 1230 * GB,
};
