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
/** Metrics with a history to chart (OVERLAY_CHART_METRICS in preferences.rs). */
/** @type {MetricId[]} */
export const CHART_METRICS = METRICS.filter(
  (id) => id !== "vram" && id !== "power",
);
export const DOCKS = /** @type {const} */ (["top", "bottom", "free"]);
export const DEFAULT_OPACITY = 60;
/** Whole-bar opacity floor (0.7.1): below 10 % the bar is as good as gone. */
export const OPACITY_FLOOR = 10;

/** A saved opacity as a whole percent in 10–100; 0.7.0 allowed 0–9, which
 * moves up to 10 (preferences.rs merge does the same when saving).
 * @param {unknown} value */
export function overlayOpacity(value) {
  const n = Number(value);
  if (value == null || value === "" || !Number.isFinite(n))
    return DEFAULT_OPACITY;
  return Math.min(100, Math.max(OPACITY_FLOOR, Math.round(n)));
}

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
  return {
    enabled: b.overlayEnabled === true,
    metrics,
    dock: DOCKS.includes(/** @type {any} */ (b.overlayDock))
      ? /** @type {typeof DOCKS[number]} */ (b.overlayDock)
      : "top",
    opacity: overlayOpacity(b.overlayOpacity),
    unlocked: b.overlayUnlocked === true,
    charts: chartMetrics(b),
    theme: typeof b.overlayTheme === "string" ? b.overlayTheme : "",
  };
}

/** Status bar appearance (0.7.5): "" follows the main window's theme, any
 * other value names a theme from definitions/themes.ts. Glassy needs the
 * main window's Acrylic backdrop, so the bar doesn't offer it.
 * @param {Record<string, { name: string, label: string }>} themes */
export function overlayThemeOptions(themes) {
  return Object.values(themes)
    .filter((t) => t.name !== "glassy")
    .map((t) => ({ value: t.name, label: t.label }));
}

/** The theme the bar uses: its own when that's a known theme it offers,
 * otherwise (unset, unknown, removed) the main window's.
 * @template {{ name: string }} T
 * @param {unknown} saved behavior.overlayTheme
 * @param {T} main the main window's theme
 * @param {Record<string, T>} themes */
export function overlayTheme(saved, main, themes) {
  if (typeof saved !== "string" || !saved || saved === "glassy") return main;
  return Object.hasOwn(themes, saved) ? themes[saved] : main;
}

/** The few colours the bar draws with, from a theme's tokens
 * (utils/theme-tokens.js): background, text, accent label, edge, chart.
 * @param {Record<string, string>} tokens */
export function overlayColors(tokens) {
  return {
    "--ob-bg": tokens.background,
    "--ob-fg": tokens.foreground,
    "--ob-accent": tokens.primary,
    "--ob-border": tokens.border,
    "--ob-chart": tokens["chart-1"] ?? tokens.primary,
  };
}

/** The metrics drawn with a chart. 0.7.3 saves the list in
 * overlayChartMetrics; before that overlayCharts switched every chart, so
 * true means all chart-capable metrics and false (or unset) none.
 * @param {Record<string, unknown>} b
 * @returns {MetricId[]} */
export function chartMetrics(b) {
  const list = b?.overlayChartMetrics;
  if (Array.isArray(list))
    return CHART_METRICS.filter((id) => list.includes(id));
  return b?.overlayCharts === true ? [...CHART_METRICS] : [];
}

/** The settings patch for a new chart list, in METRICS order. The 0.7.1
 * flag (overlayCharts) is only read, for the migration above.
 * @param {MetricId[]} charts */
export function chartsPatch(charts) {
  return {
    overlayChartMetrics: CHART_METRICS.filter((id) => charts.includes(id)),
  };
}

/** Charts after `id` is switched on or off.
 * @param {MetricId[]} charts @param {MetricId} id @param {boolean} on */
export function toggleChart(charts, id, on) {
  if (!CHART_METRICS.includes(id)) return charts;
  const rest = charts.filter((m) => m !== id);
  return CHART_METRICS.filter((m) => (m === id ? on : rest.includes(m)));
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

/** Shown metrics with the one at `from` dropped at index `to` (drag and
 * drop in Settings; `to` is the slot it lands in, counted before removal).
 * @param {MetricId[]} shown @param {number} from @param {number} to */
export function reorderMetric(shown, from, to) {
  if (from < 0 || from >= shown.length || to < 0 || to > shown.length)
    return shown;
  const at = to > from ? to - 1 : to;
  if (at === from) return shown;
  const next = [...shown];
  const [id] = next.splice(from, 1);
  next.splice(at, 0, id);
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
 *   gpuPower?: number|null, history?: any[] }} Reading  (overlay_reading in overlay.rs)
 */

/** The text of each shown metric; `null` reading: all dashes. `parts` splits
 * the two-number readings (disk R/W, network ↓/↑) so the bar can space them.
 * @param {Reading | null} r @param {MetricId[]} shown
 * @returns {{ id: MetricId, value: string, parts: string[] }[]} */
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
    disk: () => `R ${shortRate(x.diskRead)}\tW ${shortRate(x.diskWrite)}`,
    net: () => `↓${shortRate(x.netRx)}\t↑${shortRate(x.netTx)}`,
    power: () =>
      x.gpuPower == null || !Number.isFinite(x.gpuPower)
        ? "—"
        : `${Math.round(x.gpuPower)}W`,
  };
  return shown.map((id) => {
    const parts = text[id]().split("\t");
    return { id, value: parts.join(" "), parts };
  });
}

/** What each metric's chart draws from a sampler Sample (sampling.rs), and
 * its scale; `null`: no history for it (video memory, GPU power).
 * @type {Record<MetricId, { pick: (s: any) => number | null, max: number | null } | null>} */
const SERIES = {
  cpu: { pick: (s) => s.cpu ?? null, max: 100 },
  cpuTemp: { pick: (s) => s.cpu_temp ?? null, max: 100 },
  gpu: {
    pick: (s) => {
      const v = Object.values(s.gpus ?? {}).filter(
        (n) => typeof n === "number",
      );
      return v.length ? Math.max(.../** @type {number[]} */ (v)) : null;
    },
    max: 100,
  },
  gpuTemp: { pick: (s) => s.gpu_temp ?? null, max: 100 },
  ram: { pick: (s) => s.memory ?? null, max: 100 },
  vram: null,
  disk: {
    pick: (s) =>
      s.disk_read == null && s.disk_write == null
        ? null
        : (s.disk_read ?? 0) + (s.disk_write ?? 0),
    max: null,
  },
  net: {
    pick: (s) =>
      s.received == null && s.sent == null
        ? null
        : (s.received ?? 0) + (s.sent ?? 0),
    max: null,
  },
  power: null,
};

/** The chart for one metric: how to read a sample and the top of the scale
 * (rates scale to their own peak, at least 1 KB/s); `null` if it has none.
 * @param {MetricId} id @param {any[]} history */
export function chartFor(id, history) {
  const s = SERIES[id];
  if (!s) return null;
  const max =
    s.max ?? Math.max(1024, ...(history ?? []).map((x) => s.pick(x) ?? 0));
  return { pick: s.pick, max };
}
