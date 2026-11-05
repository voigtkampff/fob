// Fob status bar (v0.7.0): which readings it shows, in what order, and how
// each is written. Saved in behavior.overlay* (validated in
// src-tauri/src/preferences.rs; OVERLAY_METRICS there lists the same ids).

/** Every reading the bar can show, in the default order. No FPS: that
 * would need injecting into the game. */
export const METRICS = /** @type {const} */ ([
  "cpu",
  "cpuTemp",
  "gpu",
  "gpuTemp",
  "ram",
  "vram",
  "disk",
  "net",
  "power",
]);
/** @typedef {typeof METRICS[number]} MetricId */
export const DEFAULT_METRICS = [
  "cpu",
  "cpuTemp",
  "gpu",
  "gpuTemp",
  "ram",
  "net",
];
export const DOCKS = /** @type {const} */ (["top", "bottom", "free"]);
export const DEFAULT_OPACITY = 60;

/**
 * The saved overlay settings, with anything missing or invalid replaced by
 * its default.
 * @param {Record<string, unknown>} behavior
 */
export function overlaySettings(behavior) {
  const b = behavior ?? {};
  const list = Array.isArray(b.overlayMetrics) ? b.overlayMetrics : null;
  /** @type {MetricId[]} */
  const metrics = list
    ? list.filter(
        (id, i) =>
          METRICS.includes(/** @type {MetricId} */ (id)) &&
          list.indexOf(id) === i,
      )
    : /** @type {MetricId[]} */ ([...DEFAULT_METRICS]);
  const n = Number(b.overlayOpacity);
  return {
    enabled: b.overlayEnabled === true,
    metrics,
    dock: DOCKS.includes(/** @type {any} */ (b.overlayDock))
      ? /** @type {typeof DOCKS[number]} */ (b.overlayDock)
      : "top",
    opacity: Number.isFinite(n)
      ? Math.min(100, Math.max(0, Math.round(n)))
      : DEFAULT_OPACITY,
    unlocked: b.overlayUnlocked === true,
  };
}

/** Shown metrics with `id` switched on (added at the end) or off.
 * @param {MetricId[]} shown @param {MetricId} id @param {boolean} on */
export function toggleMetric(shown, id, on) {
  const rest = shown.filter((m) => m !== id);
  return on ? [...rest, id] : rest;
}

/** Shown metrics with `id` moved one place earlier (-1) or later (+1).
 * @param {MetricId[]} shown @param {MetricId} id @param {-1 | 1} dir */
export function moveMetric(shown, id, dir) {
  const i = shown.indexOf(id);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= shown.length) return shown;
  const next = [...shown];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

/** Every metric for the settings list: shown ones first, in their order.
 * @param {MetricId[]} shown */
export function settingsRows(shown) {
  return [
    ...shown.map((id) => ({ id, on: true })),
    ...METRICS.filter((id) => !shown.includes(id)).map((id) => ({
      id,
      on: false,
    })),
  ];
}

const GIB = 1024 ** 3;
/** Bytes/s in at most 4 characters plus a unit letter: 35K, 1.2M, 940M.
 * @param {number | null | undefined} v */
export function shortRate(v) {
  if (v == null || !Number.isFinite(v) || v < 0) return "—";
  const units = ["B", "K", "M", "G"];
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 10 || i === 0 ? Math.round(v) : v.toFixed(1)}${units[i]}`;
}
const pct = (/** @type {number | null | undefined} */ v) =>
  v == null || !Number.isFinite(v) ? "—" : `${Math.round(v)}%`;
const temp = (/** @type {number | null | undefined} */ v) =>
  v == null || !Number.isFinite(v) ? "—" : `${Math.round(v)}°C`;
const gb = (/** @type {number | null | undefined} */ v) =>
  v == null || !Number.isFinite(v) ? "—" : `${(v / GIB).toFixed(1)}G`;

/**
 * @typedef {{ cpu?: number|null, cpuTemp?: number|null, gpu?: number|null,
 *   gpuTemp?: number|null, ramUsed?: number|null, ramTotal?: number|null,
 *   vramUsed?: number|null, vramTotal?: number|null, diskRead?: number|null,
 *   diskWrite?: number|null, netRx?: number|null, netTx?: number|null,
 *   gpuPower?: number|null }} Reading  (overlay_reading in overlay.rs)
 */

/** The text of each shown metric; `null` reading: all dashes.
 * @param {Reading | null} r @param {MetricId[]} shown
 * @returns {{ id: MetricId, value: string }[]} */
export function cells(r, shown) {
  const x = r ?? {};
  /** @type {Record<MetricId, () => string>} */
  const text = {
    cpu: () => pct(x.cpu),
    cpuTemp: () => temp(x.cpuTemp),
    gpu: () => pct(x.gpu),
    gpuTemp: () => temp(x.gpuTemp),
    ram: () => gb(x.ramUsed),
    vram: () => gb(x.vramUsed),
    disk: () => `R ${shortRate(x.diskRead)} W ${shortRate(x.diskWrite)}`,
    net: () => `↓${shortRate(x.netRx)} ↑${shortRate(x.netTx)}`,
    power: () =>
      x.gpuPower == null || !Number.isFinite(x.gpuPower)
        ? "—"
        : `${Math.round(x.gpuPower)}W`,
  };
  return shown.map((id) => ({ id, value: text[id]() }));
}
