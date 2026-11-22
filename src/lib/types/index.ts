export interface Process {
  pid: number;
  ppid: number;
  name: string;
  cpu_usage: number | null;
  gpu_usage: number | null;
  memory_usage: number;
  status: string;
  user: string;
  command: string;
  threads?: number;
  environment_id?: string;
  root: string;
  virtual_memory: number;
  start_time: number;
  run_time: number;
  disk_read: number | null;
  disk_write: number | null;
  gpu_memory: number | null;
  session_id?: number;
  ports: number[];
}

export interface ResourceDetailsData {
  identity: {
    cpu_model: string | null;
    physical_cores: number | null;
    architecture: string;
    platform: string;
    os_version: string | null;
    kernel_version: string | null;
  };
  memory_unallocated: number;
  swap_total: number;
  swap_used: number;
  volumes: {
    name: string;
    mount: string;
    file_system: string;
    total: number;
    available: number;
    read_only: boolean;
    read?: number | null;
    written?: number | null;
  }[];
  interfaces: {
    name: string;
    received: number | null;
    sent: number | null;
    received_total: number;
    sent_total: number;
  }[];
}

export type CpuTopology =
  | { state: "unknown" }
  | { state: "uniform" }
  | {
      state: "hybrid";
      vendor: "intel" | "amd" | "other";
      groups: {
        kind: "performance" | "efficiency";
        cores: number;
        threads: number[];
      }[];
    };

export interface SystemStats {
  details: ResourceDetailsData;
  cpu_usage: number[];
  cpu_topology?: CpuTopology;
  memory_total: number;
  memory_used: number;
  memory_free: number;
  memory_cached: number;
  uptime: number;
  load_avg: [number, number, number];
  network_rx_bytes: number | null;
  network_tx_bytes: number | null;
  disk_total_bytes: number;
  disk_used_bytes: number;
  disk_free_bytes: number;
  disk_read_bytes?: number | null;
  disk_write_bytes?: number | null;
}

export interface ResourceSample {
  timestamp: number;
  interval: number;
  cpu: number | null;
  memory: number | null;
  received: number | null;
  sent: number | null;
  gpus: Record<string, number | null>;
  gpu?: number | null;
  cpu_temp?: number | null;
  gpu_temp?: number | null;
  disk_read?: number | null;
  disk_write?: number | null;
}

export interface ProcessTreeRow {
  process: Process;
  key: string;
  depth: number;
  childCount: number;
  expanded: boolean;
  rootIndex: number;
  familyCpuUsage: number | null;
  familyMemoryUsage: number;
  familyProcessCount: number;
}

export interface Column {
  id: keyof Process;
  label: string;
  visible: boolean;
  required?: boolean;
  numeric?: boolean;
  nullable?: boolean;
  format?: (
    value: any,
    t?: (key: string, vars?: Record<string, string | number>) => string,
  ) => string;
}

export interface Theme {
  name: string;
  label: string;
  colors: {
    base: string;
    mantle: string;
    crust: string;
    text: string;
    subtext0: string;
    subtext1: string;
    surface0: string;
    surface1: string;
    surface2: string;
    overlay0: string;
    overlay1: string;
    blue: string;
    lavender: string;
    sapphire: string;
    sky: string;
    red: string;
    maroon: string;
    peach: string;
    yellow: string;
    green: string;
    teal: string;
  };
}

export interface AppConfig {
  appearance: {
    columnVisibility: Record<string, boolean>;
  };
  behavior: {
    itemsPerPage: number;
    refreshRate: number;
    defaultStatusFilter: string;
    processView?: "flat" | "tree";
    columnOrder?: string[];
    columnWidths?: Record<string, number>;
    dashboardCards?: string[];
    processCards?: string[];
    processColumns?: number;
    menuBarEnabled: boolean;
    launchAtLogin: boolean;
    keepRunning: boolean;
    startInTray: boolean;
    miniPinned: boolean;
    language: "auto" | "en" | "zh";
    miniHotkey?: string;
    miniOpacity?: number;
    overlayEnabled?: boolean;
    overlayMetrics?: string[];
    overlayDock?: "top" | "bottom" | "free";
    overlayOpacity?: number;
    overlayLocked?: boolean;
    overlayCharts?: boolean;
    overlayChartMetrics?: string[];
    overlayTheme?: string;
    overlayHotkey?: string;
    mainHotkey?: string;
  };
}

export interface ColumnDefinition {
  id: string;
  label: string;
  visible: boolean;
  required?: boolean;
}

export interface StatusOption {
  value: string;
  label: string;
}

export interface RefreshRateOption {
  value: number;
  label: string;
}

export interface ToolBarProps {
  searchTerm: string;
  statusFilter: string;
  itemsPerPage: number;
  currentPage: number;
  totalPages: number;
  totalResults: number;
  columns: ColumnDefinition[];
  refreshRate: number;
  isFrozen: boolean;
}

export interface SortConfig {
  field: keyof Process;
  direction: "asc" | "desc";
}

export interface GpuDevice {
  id: string;
  name: string;
  vendor: string;
  utilization: number | null;
  memory_used: number | null;
  memory_total: number | null;
  memory_kind: "shared" | "dedicated" | "unknown";
  temperature: number | null;
  power_watts: number | null;
  core_clock_mhz: number | null;
  cores: number | null;
  source: string;
  /** winperf: NVML fan duty %. */
  fan_percent?: number | null;
  /** winperf: [engine type, utilization %] from Windows PDH. */
  engines?: [string, number][];
}
/** winperf: temperatures and fans (LibreHardwareMonitor, NVML). */
export interface Sensor {
  hardware: string;
  id: string;
  name: string;
  kind: "temperature" | "fan" | "control" | "voltage" | "power";
  value: number;
  source: string;
}
/** winperf sensor helper setup (Windows); see src-tauri/src/sensors/setup.rs. */
export interface SensorSetupStatus {
  supported: boolean;
  bundled: boolean;
  bundle_error: string | null;
  pawnio_installed: boolean;
  installed: boolean;
  task: "missing" | "ok" | "mismatch" | "unknown";
  update_available: boolean;
  running: boolean;
  fake: boolean;
  installed_version: string | null;
  bundled_version: string | null;
  busy: boolean;
}
export interface ThrottleReport {
  supported: boolean;
  thermal: boolean;
  thermalLog: boolean;
  prochot: boolean;
  prochotLog: boolean;
  /** MSR 0x613 advanced during the helper's last interval. */
  package: boolean;
  distanceToTjMax: number | null;
  thermalPercent: number;
  packagePercent: number;
}
export interface FrameExtra {
  sensors: {
    sensors: Sensor[];
    sources: string[];
    throttle: ThrottleReport | null;
  };
  commit: [number, number] | null;
  started?: [number, "cold" | "fast" | "hibernate"] | null;
  net_apps?: NetApp[] | null;
}
export interface NetApp {
  pid: number;
  name: string;
  rx: number;
  tx: number;
}
export interface HistoryCursor {
  epoch: string;
  timestamp: number;
}
export interface HistoryUpdate {
  reset: boolean;
  samples: ResourceSample[];
  cursor: HistoryCursor | null;
  first_timestamp: number | null;
}
export interface MonitorSnapshot {
  port_error?: string | null;
  frame: {
    captured_at: number;
    sequence: number;
    processes: Process[];
    stats: SystemStats;
    gpus: GpuDevice[];
    extra?: FrameExtra;
  };
  history: HistoryUpdate;
  paused: boolean;
  interval: number;
  error: string | null;
}
export interface OverviewSnapshot {
  captured_at: number;
  stats: SystemStats;
  gpus: GpuDevice[];
  history: ResourceSample[];
  processes: BusyProcess[];
  top_cpu: BusyProcess[];
  top_gpu: BusyProcess[];
  top_memory: BusyProcess[];
  extra: FrameExtra;
  paused: boolean;
  interval: number;
  error: string | null;
}

export type ProcessIdentity = Pick<
  Process,
  "pid" | "start_time" | "environment_id"
>;

export type BusyProcess = ProcessIdentity &
  Pick<Process, "name" | "cpu_usage" | "memory_usage" | "gpu_usage">;

/** commands::disk_health. Mirrors src-tauri/src/disk_health/mod.rs. */
export type DiskBus =
  | "nvme"
  | "sata"
  | "ata"
  | "atapi"
  | "usb"
  | "raid"
  | "scsi"
  | "sas"
  | "sd"
  | "mmc"
  | "virtual"
  | "spaces"
  | "other"
  | "unknown";
export interface NvmeCriticalWarning {
  spare_low: boolean;
  temperature: boolean;
  reliability: boolean;
  read_only: boolean;
  backup_failed: boolean;
  pmr_read_only: boolean;
  raw: number;
}
export interface NvmeHealthLog {
  critical_warning: NvmeCriticalWarning;
  temperature_c: number | null;
  available_spare: number | null;
  spare_threshold: number | null;
  percent_used: number;
  data_read_bytes: number;
  data_written_bytes: number;
  power_cycles: number;
  power_on_hours: number;
  unsafe_shutdowns: number;
  media_errors: number;
  error_log_entries: number;
  warning_temp_minutes: number;
  critical_temp_minutes: number;
}
export type NvmeState =
  | { kind: "ok"; log: NvmeHealthLog }
  | { kind: "not_asked" }
  | { kind: "unsupported" }
  | { kind: "denied" }
  | { kind: "bad_reply" }
  | { kind: "error"; code: number };
export interface AtaStats {
  temperature_c: number | null;
  percent_used: number | null;
  power_on_hours: number | null;
  power_cycles: number | null;
  data_read_bytes: number | null;
  data_written_bytes: number | null;
}
export type AtaState =
  | { kind: "ok"; stats: AtaStats }
  | { kind: "not_asked" }
  | { kind: "sleeping" }
  | { kind: "unsupported" }
  | { kind: "denied" }
  | { kind: "bad_reply" }
  | { kind: "error"; code: number };
export type DiskOperational =
  | {
      kind:
        | "ok"
        | "degraded"
        | "stressed"
        | "predictive_failure"
        | "error"
        | "non_recoverable"
        | "no_contact"
        | "lost_communication";
    }
  | { kind: "other"; code: number };
export type WindowsDiskStatus =
  | {
      kind: "ok";
      health: "healthy" | "warning" | "unhealthy" | "unknown";
      operational: DiskOperational[];
    }
  | { kind: "not_reported" }
  | { kind: "failed"; code: number };
export interface PhysicalDisk {
  number: number | null;
  model: string | null;
  bus: DiskBus;
  removable: boolean;
  volumes: string[];
  windows: WindowsDiskStatus;
  nvme: NvmeState;
  ata: AtaState;
}
export type DiskHealthReport =
  | { kind: "ok"; checked_at_ms: number; disks: PhysicalDisk[] }
  | { kind: "unavailable" }
  | { kind: "pending" };
