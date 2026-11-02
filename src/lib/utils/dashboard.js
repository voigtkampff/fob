// winperf card dashboard: pure formatting and series helpers (tested in tests/dashboard.mjs).
const GB = 1024 ** 3;

/** Bytes per second -> { value, unit }, e.g. 5_138_022 -> { value: "4.9", unit: "MB/s" }.
 * Unknown rates stay unknown ("—"), never 0.
 * @param {number | null | undefined} bytesPerSecond
 */
export function formatRate(bytesPerSecond) {
  if (
    bytesPerSecond == null ||
    !Number.isFinite(bytesPerSecond) ||
    bytesPerSecond < 0
  )
    return { value: "—", unit: "" };
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let value = bytesPerSecond;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  if (i === 0) return { value: String(Math.round(value)), unit: units[0] };
  return {
    value: value >= 100 ? value.toFixed(0) : value.toFixed(1),
    unit: units[i],
  };
}

/** @param {number} bytes */
export function formatGB(bytes, digits = 1) {
  return `${(bytes / GB).toFixed(digits)} GB`;
}

/** @param {number | null | undefined} value */
export function formatPercent(value) {
  return value == null || !Number.isFinite(value)
    ? "n/a"
    : String(Math.round(value));
}

/** Seconds -> "up 1 day, 2 hr" / "up 3 hr, 4 min" / "up 5 min".
 * @param {number} seconds
 */
export function formatUptimeShort(seconds) {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `up ${d} day${d === 1 ? "" : "s"}, ${h} hr`;
  if (h > 0) return `up ${h} hr, ${m} min`;
  return `up ${m} min`;
}

/** Average of per-core usage, or null when unknown.
 * @param {number[]} cores
 */
export function averageCpu(cores) {
  if (!cores?.length || cores.some((v) => !Number.isFinite(v))) return null;
  return Math.min(
    100,
    Math.max(0, cores.reduce((a, b) => a + b, 0) / cores.length),
  );
}

/** The system drive: C:\ on Windows, / elsewhere; falls back to the largest volume.
 * @template {{ mount: string, total: number, available: number }} V
 * @param {V[]} volumes
 * @returns {V | null}
 */
export function systemVolume(volumes) {
  if (!volumes?.length) return null;
  const norm = (/** @type {string} */ m) =>
    m.replace(/[\\/]+$/, "").toUpperCase();
  return (
    volumes.find((v) => norm(v.mount) === "C:") ??
    volumes.find((v) => v.mount === "/") ??
    [...volumes].sort((a, b) => b.total - a.total)[0]
  );
}

/** Busiest adapter wins; null when no adapter reports utilization.
 * @param {{ name: string, utilization: number | null }[]} gpus
 */
export function gpuSummary(gpus) {
  const known = (gpus ?? []).filter(
    (g) => g.utilization != null && Number.isFinite(g.utilization),
  );
  if (!known.length) return { percent: null, name: gpus?.[0]?.name ?? null };
  const top = known.reduce((a, b) =>
    /** @type {number} */ (b.utilization) >
    /** @type {number} */ (a.utilization)
      ? b
      : a,
  );
  return { percent: top.utilization, name: top.name };
}

/** @template {{ memory_usage: number, pid: number }} P
 * @param {P[]} processes
 * @returns {P[]}
 */
export function topByMemory(processes, count = 5) {
  return [...(processes ?? [])]
    .sort((a, b) => b.memory_usage - a.memory_usage || a.pid - b.pid)
    .slice(0, count);
}

/** Pull one metric out of the bounded history, keeping gaps as null.
 * @param {Record<string, any>[]} history
 * @param {string} key
 * @returns {(number | null)[]}
 */
export function series(history, key) {
  return (history ?? []).map((s) => {
    const v = s[key];
    return typeof v === "number" && Number.isFinite(v) ? v : null;
  });
}

/** Keep only the newest `size` points (a fixed-width window for small charts).
 * @template T
 * @param {T[]} values
 * @param {number} size
 */
export function lastN(values, size) {
  return values.length > size ? values.slice(values.length - size) : values;
}

/** Scale for a shared network chart: at least 1 KB/s so idle lines sit at the bottom.
 * @param {(number | null)[][]} lines
 */
export function chartMax(lines, floor = 1024) {
  return Math.max(floor, ...lines.flat().map((v) => v ?? 0));
}

/** SVG path for a sparkline. Points are evenly spaced over `slots` slots,
 * aligned right; null values break the line.
 * @param {(number | null)[]} values
 * @param {{ width: number, height: number, max: number, slots?: number }} box
 */
export function sparklinePath(
  values,
  { width, height, max, slots = values.length },
) {
  if (!values.length || max <= 0) return "";
  const step = slots > 1 ? width / (slots - 1) : 0;
  const offset = slots - values.length;
  let path = "";
  let pen = false;
  values.forEach((v, i) => {
    if (v == null) {
      pen = false;
      return;
    }
    const x = (offset + i) * step;
    const y = height - (Math.min(Math.max(v, 0), max) / max) * height;
    path += `${pen ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`;
    pen = true;
  });
  return path;
}

/** Top `count` items by a numeric key (descending); items without a value are skipped.
 * @template T
 * @param {T[]} items
 * @param {(item: T) => number | null | undefined} key
 * @returns {T[]}
 */
export function topBy(items, key, count = 10) {
  return (items ?? [])
    .map((item) => ({ item, value: key(item) }))
    .filter((x) => typeof x.value === "number" && Number.isFinite(x.value))
    .sort(
      (a, b) =>
        /** @type {number} */ (b.value) - /** @type {number} */ (a.value),
    )
    .slice(0, count)
    .map((x) => x.item);
}

/** Pick headline readings for the Temp & fan card.
 * @param {{ hardware: string, name: string, kind: "temperature" | "fan" | "control", value: number }[]} sensors
 */
export function sensorSummary(sensors) {
  const list = sensors ?? [];
  const temps = list.filter((s) => s.kind === "temperature");
  const isCpu = (/** @type {{ hardware: string, name: string }} */ s) =>
    /cpu|core|ryzen|intel|package|tctl/i.test(`${s.hardware} ${s.name}`) &&
    !/gpu|nvidia|radeon|geforce/i.test(`${s.hardware} ${s.name}`);
  const cpuTemp =
    temps.find((s) => /package|tctl|tdie/i.test(s.name) && isCpu(s)) ??
    temps.find(isCpu) ??
    null;
  const gpuTemp =
    temps.find((s) =>
      /gpu|nvidia|radeon|geforce/i.test(`${s.hardware} ${s.name}`),
    ) ?? null;
  const fans = list.filter((s) => s.kind === "fan" && s.value > 0);
  const fan = fans.length
    ? fans.reduce((a, b) => (b.value > a.value ? b : a))
    : (list.find((s) => s.kind === "control") ?? null);
  return { cpuTemp, gpuTemp, fan, count: list.length };
}

/** Group sensors by hardware, keeping first-seen order.
 * @template {{ hardware: string }} S
 * @param {S[]} sensors
 * @returns {{ hardware: string, sensors: S[] }[]}
 */
export function groupByHardware(sensors) {
  /** @type {Map<string, S[]>} */
  const groups = new Map();
  for (const s of sensors ?? []) {
    if (!groups.has(s.hardware)) groups.set(s.hardware, []);
    /** @type {S[]} */ (groups.get(s.hardware)).push(s);
  }
  return [...groups].map(([hardware, list]) => ({ hardware, sensors: list }));
}

/** @param {{ kind: string, value: number }} s */
export function formatSensor(s) {
  if (s.kind === "temperature") return `${s.value.toFixed(0)} °C`;
  if (s.kind === "fan") return `${Math.round(s.value)} RPM`;
  return `${Math.round(s.value)} %`;
}
