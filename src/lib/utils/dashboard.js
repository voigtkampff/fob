import { tEn } from "../i18n/strings.js";

/** @typedef {(key: string, vars?: Record<string, string | number>) => string} Translate */

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
 * @param {Translate} [t]
 */
export function formatUptimeShort(seconds, t = tEn) {
  return t("uptime.up", { d: formatDuration(seconds, t) });
}

/** "2 days, 21 hr" / "3 hr, 4 min" / "5 min" (or the locale's form).
 * @param {number} seconds
 * @param {Translate} [t]
 */
export function formatDuration(seconds, t = tEn) {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return t(d === 1 ? "dur.days" : "dur.daysPlural", { d, h });
  if (h > 0) return t("dur.hours", { h, m });
  return t("dur.minutes", { m });
}

/** What the uptime chip shows. With Windows Fast Startup the kernel uptime
 * spans days the PC was "shut down"; `startedAt` (Unix s, from the boot event
 * log) is the last real start, so prefer it when known.
 * @param {number} kernelUptime seconds
 * @param {number | null | undefined} startedAt Unix seconds
 * @param {number} nowSec Unix seconds
 */
export function uptimeInfo(kernelUptime, startedAt, nowSec) {
  if (startedAt == null || !Number.isFinite(startedAt))
    return { seconds: kernelUptime, sinceStart: false, kernel: kernelUptime };
  const since = Math.max(0, Math.round(nowSec - startedAt));
  return {
    seconds: Math.min(kernelUptime, since),
    sinceStart: true,
    kernel: kernelUptime,
  };
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

/** Natural order for sensor names ("Fan #2" before "Fan #10"). */
const natural = new Intl.Collator("en", { numeric: true, sensitivity: "base" });

/** Fan RPM sensors grouped by hardware (board first-seen order), names in
 * natural order. 0 RPM fans are kept: stopped or not connected.
 * @template {{ hardware: string, name: string, kind: string, value: number }} S
 * @param {S[]} sensors
 * @returns {{ hardware: string, fans: S[] }[]}
 */
export function fanGroups(sensors) {
  return groupByHardware((sensors ?? []).filter((s) => s.kind === "fan"))
    .map(({ hardware, sensors: fans }) => ({
      hardware,
      fans: [...fans].sort((a, b) => natural.compare(a.name, b.name)),
    }))
    .filter((g) => g.fans.length);
}

/** "1102 · 1349 RPM +2" for the spinning fans, or "" when none spin.
 * @param {{ kind: string, value: number }[]} sensors
 */
export function fanText(sensors, max = 3) {
  const spinning = fanGroups(/** @type {any[]} */ (sensors))
    .flatMap((g) => g.fans)
    .filter((f) => f.value > 0);
  if (!spinning.length) return "";
  const shown = spinning.slice(0, max).map((f) => Math.round(f.value));
  const more = spinning.length - shown.length;
  return `${shown.join(" · ")} RPM${more > 0 ? ` +${more}` : ""}`;
}

/** Sensors are sorted temperature, fan, duty within a hardware group.
 * @template {{ kind: string, name: string }} S
 * @param {S[]} list
 */
export function byKind(list) {
  const rank = { temperature: 0, fan: 1, control: 2 };
  return [...list].sort(
    (a, b) =>
      (rank[/** @type {keyof typeof rank} */ (a.kind)] ?? 3) -
        (rank[/** @type {keyof typeof rank} */ (b.kind)] ?? 3) ||
      natural.compare(a.name, b.name),
  );
}

/** What the user has to do for fan/temperature readings, from the setup status.
 * "none": nothing to offer (not Windows / dev build / already working).
 * @param {{ supported: boolean, bundled: boolean, installed: boolean, task: string, update_available: boolean, running: boolean } | null | undefined} status
 * @returns {"none" | "missing-files" | "enable" | "repair" | "update" | "starting" | "ready"}
 */
export function setupState(status) {
  if (!status || !status.supported) return "none";
  const done = status.installed && status.task === "ok";
  if (done && status.update_available)
    return status.bundled ? "update" : "ready";
  if (done) return status.running ? "ready" : "starting";
  if (!status.bundled) return "missing-files";
  return status.installed || status.task !== "missing" ? "repair" : "enable";
}

/** The steps the elevated setup will take, shown before anything runs.
 * @param {{ pawnio_installed: boolean, installed: boolean, task: string }} status
 * @param {Translate} [t]
 */
export function setupSteps(status, t = tEn) {
  const steps = [];
  if (!status.pawnio_installed) steps.push(t("steps.pawnio"));
  steps.push(t(status.installed ? "steps.replace" : "steps.copy"));
  steps.push(t(status.task === "missing" ? "steps.task" : "steps.retask"));
  return steps;
}

/** Per-app network rows for the network detail: apps grouped by name (a
 * browser's processes add up), kept for `keepMs` after their last traffic so
 * the list doesn't jump, busiest first.
 * @param {Map<string, { name: string, rx: number, tx: number, seen: number }>} prev
 * @param {{ name: string, rx: number, tx: number }[]} apps current rates (B/s)
 * @param {number} now ms
 */
export function mergeNetApps(prev, apps, now, keepMs = 30000) {
  /** @type {Map<string, { name: string, rx: number, tx: number, seen: number }>} */
  const next = new Map();
  for (const [name, row] of prev)
    if (now - row.seen <= keepMs) next.set(name, { ...row, rx: 0, tx: 0 });
  for (const app of apps) {
    const row = next.get(app.name) ?? {
      name: app.name,
      rx: 0,
      tx: 0,
      seen: now,
    };
    row.rx += app.rx;
    row.tx += app.tx;
    if (app.rx + app.tx > 0) row.seen = now;
    next.set(app.name, row);
  }
  return next;
}

/** @param {Map<string, { name: string, rx: number, tx: number, seen: number }>} rows */
export function sortedNetRows(rows) {
  return [...rows.values()].sort(
    (a, b) => b.rx + b.tx - (a.rx + a.tx) || natural.compare(a.name, b.name),
  );
}
