// Fob status bar (v0.7.0): which readings it shows, in what order, and how
// each is written. Saved in behavior.overlay* (validated in
// src-tauri/src/preferences.rs; OVERLAY_METRICS there lists the same ids).
import { normalizeOrder } from "./layout-order.js";

/** Every field the bar can show. A field belongs to one block; the block
 * order is saved separately (overlayOrder), so a field never carries an
 * order of its own. No FPS: that would need injecting into the game. */
export const METRICS = /** @type {const} */ ([
  "cpu",
  "cpuPower",
  "cpuTemp",
  "cpuVoltage",
  "cpuClock",
  "gpu",
  "power",
  "gpuTemp",
  "ram",
  "vram",
  "disk",
  "net",
]);
/** @typedef {typeof METRICS[number]} MetricId */

/** The bar's blocks, in their default order, with the fields each one draws
 * and the switches its settings section offers. Field order inside a block
 * is fixed: usage (carrying power) → temperature → voltage → clock for the
 * CPU, usage (carrying power) → temperature for the GPU.
 * `rows` lists the settings switches a block opens: metric rows toggle the
 * field in overlayMetrics, chart rows toggle it in overlayChartMetrics.
 * A block with one metric row and no chart row is a single-row block
 * (memory, video memory, disk, network).
 * @type {{ id: string, label: string, fields: MetricId[],
 *   rows: { kind: "metric" | "chart", id: MetricId, label: string }[] }[]} */
export const BLOCKS = [
  {
    id: "cpu",
    label: "overlay.metric.cpu",
    fields: ["cpu", "cpuPower", "cpuTemp", "cpuVoltage", "cpuClock"],
    rows: [
      { kind: "metric", id: "cpu", label: "overlay.child.usage" },
      { kind: "chart", id: "cpu", label: "overlay.child.chart" },
      { kind: "metric", id: "cpuPower", label: "overlay.metric.cpuPower" },
      { kind: "metric", id: "cpuTemp", label: "overlay.child.temp" },
      { kind: "metric", id: "cpuVoltage", label: "overlay.child.voltage" },
      { kind: "metric", id: "cpuClock", label: "overlay.child.clock" },
    ],
  },
  {
    id: "gpu",
    label: "overlay.metric.gpu",
    fields: ["gpu", "power", "gpuTemp"],
    rows: [
      { kind: "metric", id: "gpu", label: "overlay.child.usage" },
      { kind: "chart", id: "gpu", label: "overlay.child.chart" },
      { kind: "metric", id: "power", label: "overlay.metric.power" },
      { kind: "metric", id: "gpuTemp", label: "overlay.child.temp" },
    ],
  },
  {
    id: "ram",
    label: "overlay.metric.ram",
    fields: ["ram"],
    rows: [{ kind: "metric", id: "ram", label: "overlay.metric.ram" }],
  },
  {
    id: "vram",
    label: "overlay.metric.vram",
    fields: ["vram"],
    rows: [{ kind: "metric", id: "vram", label: "overlay.metric.vram" }],
  },
  {
    id: "disk",
    label: "overlay.metric.disk",
    fields: ["disk"],
    rows: [{ kind: "metric", id: "disk", label: "overlay.metric.disk" }],
  },
  {
    id: "net",
    label: "overlay.metric.net",
    fields: ["net"],
    rows: [{ kind: "metric", id: "net", label: "overlay.metric.net" }],
  },
];
export const BLOCK_IDS = BLOCKS.map((block) => block.id);
/** @type {Record<string, typeof BLOCKS[number]>} */
const BLOCK_OF = Object.fromEntries(BLOCKS.map((block) => [block.id, block]));

export const DEFAULT_METRICS = [
  "cpu",
  "cpuPower",
  "cpuTemp",
  "cpuVoltage",
  "cpuClock",
  "gpu",
  "power",
  "gpuTemp",
  "ram",
  "net",
];
/** Metrics with a history to chart (OVERLAY_CHART_METRICS in preferences.rs).
 * Charts follow CPU/GPU usage and the single readings that already had one;
 * power, temperature, voltage and frequency never draw a curve. */
/** @type {MetricId[]} */
export const CHART_METRICS = ["cpu", "gpu", "ram", "disk", "net"];
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
  // One flat enabled set, in METRICS order: the block order lives in
  // overlayOrder, so a field never has a position of its own. The order of
  // the saved list still decides the block order when overlayOrder is
  // missing (pre-0.8.13 configuration).
  /** @type {MetricId[]} */
  const savedFields = list
    ? list.filter((id) => METRICS.includes(/** @type {MetricId} */ (id)))
    : /** @type {MetricId[]} */ ([...DEFAULT_METRICS]);
  /** @type {MetricId[]} */
  const metrics = list
    ? METRICS.filter((id) => list.includes(id))
    : /** @type {MetricId[]} */ ([...DEFAULT_METRICS]);
  return {
    enabled: b.overlayEnabled === true,
    metrics,
    order: overlayBlockOrder(b.overlayOrder, savedFields),
    dock: DOCKS.includes(/** @type {any} */ (b.overlayDock))
      ? /** @type {typeof DOCKS[number]} */ (b.overlayDock)
      : "top",
    opacity: overlayOpacity(b.overlayOpacity),
    // 0.7.5: overlayUnlocked is never read, so everyone starts unlocked.
    locked: b.overlayLocked === true,
    charts: chartMetrics(b),
    theme: typeof b.overlayTheme === "string" ? b.overlayTheme : "",
  };
}

/** The block a saved field belongs to, or null for an unknown id.
 * @param {string} id */
function blockOf(id) {
  return Object.values(BLOCK_OF).find((block) =>
    block.fields.includes(/** @type {MetricId} */ (id)),
  )?.id;
}

/** A block order derived from the old flat field list: the first block seen
 * wins, and blocks that never appeared follow in their default places. Old
 * configurations stored only overlayMetrics, so this keeps their intent.
 * @param {MetricId[]} metrics */
function derivedBlockOrder(metrics) {
  /** @type {string[]} */
  const seen = [];
  for (const id of metrics) {
    const block = blockOf(id);
    if (block && !seen.includes(block)) seen.push(block);
  }
  // Blocks that never appeared follow in their default order, so a field
  // mentioned late cannot push an earlier block out of place.
  for (const block of BLOCK_IDS) {
    if (!seen.includes(block)) seen.push(block);
  }
  return seen;
}

/** A saved block order made valid: unknown or repeated ids dropped, missing
 * blocks back at their default place. Without a saved order the old field
 * order decides, so upgrading keeps the user's arrangement.
 * @param {unknown} saved @param {MetricId[]} metrics
 * @returns {string[]} */
export function overlayBlockOrder(saved, metrics) {
  if (Array.isArray(saved))
    return normalizeOrder(
      saved.filter((id) => typeof id === "string" && BLOCK_IDS.includes(id)),
      BLOCK_IDS,
    );
  return derivedBlockOrder(metrics);
}

/** The settings sections: one block per row, with the switches it opens
 * indented under it. Single-row blocks (memory, video memory, disk, network)
 * keep their field switch and chart toggle on the row itself.
 * @param {MetricId[]} metrics @param {string[]} order @param {MetricId[]} charts */
export function settingsBlocks(metrics, order, charts) {
  const on = new Set(metrics);
  const charted = new Set(charts);
  return order
    .map((id) => BLOCK_OF[id])
    .filter(Boolean)
    .map((block) => {
      const rows = block.rows.map((row) => ({
        ...row,
        on: row.kind === "chart" ? charted.has(row.id) : on.has(row.id),
      }));
      const fields = rows.filter((row) => row.kind === "metric");
      const single = fields.length === 1;
      const field = fields[0]?.id;
      return {
        id: block.id,
        label: block.label,
        single,
        field: single ? field : null,
        on: fields.some((row) => row.on),
        chartable: single && chartable(field),
        charted: single && charted.has(field),
        rows: single ? [] : rows,
      };
    });
}

/** Whether a field has a chart (CHART_METRICS).
 * @param {MetricId | null | undefined} id */
function chartable(id) {
  return id != null && CHART_METRICS.includes(id);
}

/** Enabled fields with one block turned off, or back on at its first field
 * (usage for CPU/GPU; the only field for a single block). `on` keeps the
 * other fields of that block untouched when enabling.
 * @param {MetricId[]} metrics @param {string} id @param {boolean} on */
export function toggleBlock(metrics, id, on) {
  const block = BLOCK_OF[id];
  if (!block) return metrics;
  const rest = metrics.filter((field) => !block.fields.includes(field));
  if (!on) return rest;
  const first = block.fields[0];
  return METRICS.filter(
    (field) =>
      rest.includes(field) || metrics.includes(field) || field === first,
  );
}

/** One block moved earlier (-1) or later (+1) in the block order.
 * @param {string[]} order @param {string} id @param {-1 | 1} dir */
export function moveBlock(order, id, dir) {
  const i = order.indexOf(id);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= order.length) return order;
  const next = [...order];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
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
 * @template {T | null} M
 * @param {unknown} saved behavior.overlayTheme
 * @param {M} main the main window's theme (null: "following")
 * @param {Record<string, T>} themes
 * @returns {T | M} */
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

/** How far (px, + is down) to move text in a `line-height: 1` box so its
 * ink is centred: the browser centres the font's ascent + descent, not the
 * glyphs, so fonts with tall metrics (Microsoft YaHei UI) sit off centre.
 * @param {number} size font size in px
 * @param {{ fontBoundingBoxAscent: number, fontBoundingBoxDescent: number,
 *   actualBoundingBoxAscent: number, actualBoundingBoxDescent: number }} m
 *   canvas TextMetrics for sample text in that font */
export function inkShift(size, m) {
  const baseline =
    (size - (m.fontBoundingBoxAscent + m.fontBoundingBoxDescent)) / 2 +
    m.fontBoundingBoxAscent;
  const ink =
    baseline - (m.actualBoundingBoxAscent - m.actualBoundingBoxDescent) / 2;
  return size / 2 - ink;
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

/** The slot (0..rects.length) a row dragged to `y` lands in: before the
 * first row whose middle is below `y`, else after the last.
 * @param {number} y @param {{ top: number, bottom: number }[]} rects */
export function dropSlot(y, rects) {
  const i = rects.findIndex((r) => y < (r.top + r.bottom) / 2);
  return i < 0 ? rects.length : i;
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
const volts = (/** @type {number | null | undefined} */ v) =>
  v == null || !Number.isFinite(v) ? "—" : `${v.toFixed(2)}V`;
/** Effective frequency from MHz: 3.76GHz. Its unit says what it is, so the
 * bar shows no prefix for it. */
const ghz = (/** @type {number | null | undefined} */ v) =>
  v == null || !Number.isFinite(v) || v <= 0
    ? "—"
    : `${(v / 1000).toFixed(2)}GHz`;
const watts = (/** @type {number | null | undefined} */ v) =>
  v == null || !Number.isFinite(v) ? "—" : `${Math.round(v)}W`;
const gb = (/** @type {number | null | undefined} */ v) =>
  v == null || !Number.isFinite(v) ? "—" : `${(v / GIB).toFixed(1)}G`;

/**
 * @typedef {{
 *   cpu?: number|null, cpuTemp?: number|null, cpuVoltage?: number|null,
 *   cpuVoltageSource?: "measured"|"requested"|null,
 *   cpuClock?: number|null, cpuPower?: number|null,
 *   gpu?: number|null,
 *   gpuTemp?: number|null, ramUsed?: number|null, ramTotal?: number|null,
 *   vramUsed?: number|null, vramTotal?: number|null, diskRead?: number|null,
 *   diskWrite?: number|null, netRx?: number|null, netTx?: number|null,
 *   gpuPower?: number|null, throttle?: "power"|"thermal"|null,
 *   history?: any[]
 * }} Reading  (overlay_reading in overlay.rs)
 */

/** The cells the bar draws, in block order. A block draws only the fields
 * that are switched on, in its fixed internal order: usage (carrying power)
 * → temperature → voltage → clock for the CPU, usage (carrying power) →
 * temperature for the GPU. Dragging reorders blocks, never fields, so this
 * function reads the block order, not the order of the enabled set.
 *
 * Power never gets a cell of its own: CPU package power follows CPU usage and
 * GPU power follows GPU usage (`CPU 15% 45W`). Leaving usage off still shows
 * a power-only cell (`CPU 45W`); a missing power number is left out instead
 * of adding a second dash. Charts only ever read a usage cell's id.
 *
 * Each metric carries its short name (0.8.13): `CPU`, `GPU`, `CLK`, and
 * `Vcore`/`VID` for the voltage, which follows its source. Temperature is
 * bare inside its block (`47°C`); memory, disk and network keep their names.
 * `label: ""` means no name; null means `overlay.short.<id>`.
 * @param {Reading | null} r @param {MetricId[]} shown
 * @param {string[]} [order] block order (defaults to the old flat order)
 * @returns {{ id: MetricId, value: string, parts: string[],
 *   label: string | null }[]} */
export function cells(r, shown, order = derivedBlockOrder(shown)) {
  const x = r ?? {};
  const on = new Set(shown);
  /** The voltage prefix only when the source is known and the value exists;
   * no prefix beats a wrong claim. */
  const voltLabel =
    x.cpuVoltage == null || !Number.isFinite(x.cpuVoltage)
      ? ""
      : x.cpuVoltageSource === "measured"
        ? "Vcore"
        : x.cpuVoltageSource === "requested"
          ? "VID"
          : "";
  /** @type {Record<MetricId, () => string>} */
  const text = {
    cpu: () => pct(x.cpu),
    cpuTemp: () => temp(x.cpuTemp),
    cpuVoltage: () => volts(x.cpuVoltage),
    cpuClock: () => ghz(x.cpuClock),
    cpuPower: () => watts(x.cpuPower),
    gpu: () => pct(x.gpu),
    gpuTemp: () => temp(x.gpuTemp),
    ram: () => gb(x.ramUsed),
    vram: () => gb(x.vramUsed),
    disk: () => `R ${shortRate(x.diskRead)}	W ${shortRate(x.diskWrite)}`,
    net: () => `↓${shortRate(x.netRx)}	↑${shortRate(x.netTx)}`,
    power: () => watts(x.gpuPower),
  };
  /** Names the bar writes itself, or null for the i18n table. */
  /** @type {Partial<Record<MetricId, string>>} */
  const labels = {
    cpu: "CPU",
    cpuPower: "CPU",
    gpu: "GPU",
    power: "GPU",
    cpuClock: "CLK",
    cpuVoltage: voltLabel,
    cpuTemp: "",
    gpuTemp: "",
  };
  /** @type {{ id: MetricId, value: string, parts: string[], label: string | null }[]} */
  const out = [];
  /** @param {MetricId} id */
  const pushScalar = (id) => {
    const parts = text[id]().split("	");
    out.push({
      id,
      value: parts.join(" "),
      parts,
      label: labels[id] ?? null,
    });
  };
  for (const blockId of order) {
    const block = BLOCK_OF[blockId];
    if (!block) continue;
    if (blockId === "cpu" || blockId === "gpu") {
      const usageId = /** @type {MetricId} */ (blockId);
      const powerId = /** @type {MetricId} */ (
        blockId === "cpu" ? "cpuPower" : "power"
      );
      const usageOn = on.has(usageId);
      const powerOn = on.has(powerId);
      if (usageOn || powerOn) {
        /** @type {string[]} */
        const parts = [];
        if (usageOn && text[usageId]() !== "—") parts.push(text[usageId]());
        if (powerOn && text[powerId]() !== "—") parts.push(text[powerId]());
        if (!parts.length) parts.push("—");
        const id = usageOn ? usageId : powerId;
        out.push({
          id,
          value: parts.join(" "),
          parts,
          label: labels[id] ?? null,
        });
      }
      for (const id of block.fields) {
        if (id === usageId || id === powerId) continue;
        if (on.has(id)) pushScalar(id);
      }
      continue;
    }
    for (const id of block.fields) if (on.has(id)) pushScalar(id);
  }
  return out;
}

/** What each metric's chart draws from a sampler Sample (sampling.rs), and
 * its scale; `null`: no history for it (both power readings, CPU voltage,
 * CPU frequency, video memory).
 * @type {Record<MetricId, { pick: (s: any) => number | null, max: number | null } | null>} */
const SERIES = {
  cpu: { pick: (s) => s.cpu ?? null, max: 100 },
  cpuTemp: { pick: (s) => s.cpu_temp ?? null, max: 100 },
  cpuVoltage: null,
  cpuClock: null,
  cpuPower: null,
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
