import type { Column } from "$lib/types";
import { formatMemorySize } from "$lib/utils";
import { formatHms, formatRate } from "$lib/utils/dashboard.js";

/** Bytes/s as "1.2 MB/s"; no reading is "—", never 0. */
export const rate = (v: number | null) => {
  const { value, unit } = formatRate(v);
  return unit ? `${value} ${unit}` : value;
};

export let column_definitions: Column[] = [
  { id: "name", label: "Process Name", visible: true, required: true },
  { id: "pid", label: "PID", visible: true, required: false },
  {
    id: "status",
    label: "Status",
    visible: true,
  },
  { id: "user", label: "User", visible: true },
  {
    id: "cpu_usage",
    label: "CPU %",
    visible: true,
    numeric: true,
    format: (v) => v.toFixed(1) + "%",
  },
  {
    id: "memory_usage",
    label: "RAM",
    visible: true,
    numeric: true,
    format: (v) => (v / (1024 * 1024)).toFixed(1) + " MB",
  },
  {
    id: "gpu_usage",
    label: "GPU %",
    visible: true,
    numeric: true,
    nullable: true,
    format: (v) => (v == null ? "—" : `${v.toFixed(1)}%`),
  },
  {
    id: "disk_read",
    label: "Disk read",
    visible: true,
    numeric: true,
    nullable: true,
    format: rate,
  },
  {
    id: "disk_write",
    label: "Disk write",
    visible: true,
    numeric: true,
    nullable: true,
    format: rate,
  },
  {
    id: "gpu_memory",
    label: "Dedicated VRAM",
    visible: false,
    numeric: true,
    nullable: true,
    format: (v) => (v == null ? "—" : `${(v / (1024 * 1024)).toFixed(0)} MB`),
  },
  {
    id: "virtual_memory",
    label: "VIRT",
    visible: true,
    numeric: true,
    format: (v) => formatMemorySize(v),
  },
  { id: "ppid", label: "Parent PID", visible: false },
  { id: "root", label: "Root", visible: false },
  { id: "command", label: "Command", visible: false },
  { id: "environ", label: "Environment Variables", visible: false },
  { id: "session_id", label: "Session ID", visible: false },
  {
    id: "start_time",
    label: "Start Time",
    visible: false,
    format: (v) => new Date(v * 1000).toLocaleString(), // v is the time where the process was started (in seconds) from epoch
  },
  {
    id: "run_time",
    label: "Run Time",
    visible: true,
    numeric: true,
    // v: seconds the process has run. Both languages show "4h 18m 25s":
    // shorter than 时分秒 and the same width, so one column width fits both.
    format: (v) => formatHms(v),
  },
];
