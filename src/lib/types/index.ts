// Create a new types file to centralize interfaces
export interface Process {
  pid: number;
  ppid: number;
  name: string;
  cpu_usage: number;
  gpu_usage: number | null;
  memory_usage: number;
  status: string;
  user: string;
  command: string;
  threads?: number;
  environ: string[];
  root: string;
  virtual_memory: number;
  start_time: number;
  run_time: number;
  disk_usage: [number, number]; // [read_bytes, written_bytes]
  session_id?: number;
  /** Local bound ports (TCP LISTEN + UDP). Empty unless a port search is active. */
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
    /** Fob: bytes/s; null when the drive reports no I/O counters. */
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

export interface SystemStats {
  details: ResourceDetailsData;
  cpu_usage: number[];
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
  /** Fob: storage read / write, bytes/s (null: not reported). */
  disk_read_bytes?: number | null;
  disk_write_bytes?: number | null;
}

/** Volatile resource observations, timed at completion of the existing read. */
export interface ResourceSample {
  timestamp: number;
  interval: number;
  cpu: number | null;
  memory: number | null;
  received: number | null;
  sent: number | null;
  gpus: Record<string, number | null>;
  gpu?: number | null;
  /** Fob: Temp & fan card headline readings, °C. */
  cpu_temp?: number | null;
  gpu_temp?: number | null;
  /** Fob: storage read / write, bytes/s. */
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
  familyCpuUsage: number;
  familyMemoryUsage: number;
  familyProcessCount: number;
}

export interface Column {
  id: keyof Process;
  label: string;
  visible: boolean;
  required?: boolean;
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
    menuBarEnabled: boolean;
    keepRunning: boolean;
    /** winperf: launch with only the tray icon (mini window mode). */
    startInTray: boolean;
    /** Fob: mini window stays open on focus loss (pin button). */
    miniPinned: boolean;
    /** "auto" follows the Windows display language. */
    language: "auto" | "en" | "zh";
    /** Fob: global hotkey that toggles the mini window ("" = off). */
    miniHotkey?: string;
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
  /** e.g. "/lpc/example/0/fan/1"; "" when the source has none. */
  id: string;
  name: string;
  kind: "temperature" | "fan" | "control";
  value: number;
  source: string;
}
/** winperf sensor helper setup (Windows); see src-tauri/src/sensors/setup.rs. */
export interface SensorSetupStatus {
  supported: boolean;
  bundled: boolean;
  pawnio_installed: boolean;
  installed: boolean;
  task: "missing" | "ok" | "mismatch" | "unknown";
  update_available: boolean;
  running: boolean;
  fake: boolean;
}
export interface FrameExtra {
  sensors: { sensors: Sensor[]; sources: string[] };
  /** Windows commit charge [used, limit] in bytes. */
  commit: [number, number] | null;
  /** Last power-on: Unix seconds and kind; null = only kernel uptime known. */
  started?: [number, "cold" | "fast" | "hibernate"] | null;
  /** Per-process network rates (bytes/s) from the sensor helper's ETW session. */
  net_apps?: NetApp[] | null;
}
export interface NetApp {
  pid: number;
  name: string;
  rx: number;
  tx: number;
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
  history: ResourceSample[];
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

export type BusyProcess = Pick<
  Process,
  "pid" | "start_time" | "name" | "cpu_usage" | "memory_usage" | "gpu_usage"
>;
