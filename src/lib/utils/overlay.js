import { normalizeOrder, previewOrder } from "./layout-order.js";

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

export const FIELD_SLOTS = /** @type {const} */ ({
  cpu: ["cpu", "cpuPower", "cpuTemp", "cpuVoltage", "cpuClock"],
  gpu: ["gpu", "power", "gpuTemp"],
});
export const DEFAULT_FIELD_ORDER = {
  cpu: [...FIELD_SLOTS.cpu],
  gpu: [...FIELD_SLOTS.gpu],
};
export const FIELD_TIPS = {
  cpu: "overlay.child.usage",
  cpuPower: "overlay.child.power",
  cpuTemp: "overlay.child.temp",
  cpuVoltage: "overlay.child.voltage",
  cpuClock: "overlay.child.clock",
  gpu: "overlay.child.usage",
  power: "overlay.child.power",
  gpuTemp: "overlay.child.temp",
  ram: "overlay.metric.ram",
  vram: "overlay.metric.vram",
  disk: "overlay.metric.disk",
  net: "overlay.metric.net",
};

/** @type {{ id: string, label: string, fields: MetricId[],
 *   rows: { id: MetricId, label: string }[] }[]} */
export const BLOCKS = [
  {
    id: "cpu",
    label: "overlay.metric.cpu",
    fields: ["cpu", "cpuPower", "cpuTemp", "cpuVoltage", "cpuClock"],
    rows: [
      { id: "cpu", label: "overlay.child.usage" },
      { id: "cpuPower", label: "overlay.metric.cpuPower" },
      { id: "cpuTemp", label: "overlay.child.temp" },
      { id: "cpuVoltage", label: "overlay.child.voltage" },
      { id: "cpuClock", label: "overlay.child.clock" },
    ],
  },
  {
    id: "gpu",
    label: "overlay.metric.gpu",
    fields: ["gpu", "power", "gpuTemp"],
    rows: [
      { id: "gpu", label: "overlay.child.usage" },
      { id: "power", label: "overlay.metric.power" },
      { id: "gpuTemp", label: "overlay.child.temp" },
    ],
  },
  {
    id: "ram",
    label: "overlay.metric.ram",
    fields: ["ram"],
    rows: [{ id: "ram", label: "overlay.metric.ram" }],
  },
  {
    id: "vram",
    label: "overlay.metric.vram",
    fields: ["vram"],
    rows: [{ id: "vram", label: "overlay.metric.vram" }],
  },
  {
    id: "disk",
    label: "overlay.metric.disk",
    fields: ["disk"],
    rows: [{ id: "disk", label: "overlay.metric.disk" }],
  },
  {
    id: "net",
    label: "overlay.metric.net",
    fields: ["net"],
    rows: [{ id: "net", label: "overlay.metric.net" }],
  },
];
export const BLOCK_IDS = BLOCKS.map((block) => block.id);
export const DEFAULT_BLOCKS = ["cpu", "gpu", "ram", "net"];
/** @type {Record<string, typeof BLOCKS[number]>} */
const BLOCK_OF = Object.fromEntries(BLOCKS.map((block) => [block.id, block]));
/** @type {Record<string, string>} */
const FIELD_LABELS = Object.fromEntries(
  BLOCKS.flatMap((block) => block.rows.map((row) => [row.id, row.label])),
);

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
/** @type {MetricId[]} */
export const CHART_METRICS = ["cpu", "gpu", "ram", "disk", "net"];
export const DOCKS = /** @type {const} */ (["top", "bottom", "free"]);
export const DEFAULT_OPACITY = 60;
export const OPACITY_FLOOR = 10;

/**
 * @param {unknown} value */
export function overlayOpacity(value) {
  const n = Number(value);
  if (value == null || value === "" || !Number.isFinite(n))
    return DEFAULT_OPACITY;
  return Math.min(100, Math.max(OPACITY_FLOOR, Math.round(n)));
}

/** @param {unknown} saved */
export function overlayFieldOrder(saved = undefined) {
  const source =
    saved && typeof saved === "object"
      ? /** @type {Record<string, unknown>} */ (saved)
      : {};
  /** @type {Record<string, MetricId[]>} */
  const order = {};
  for (const [family, slots] of Object.entries(FIELD_SLOTS)) {
    /** @type {MetricId[]} */
    const ids = [...slots];
    const list = Array.isArray(source[family])
      ? source[family].filter(
          (id) =>
            typeof id === "string" &&
            ids.includes(/** @type {MetricId} */ (id)),
        )
      : slots;
    const seen = [...new Set(list)];
    order[family] = [...seen, ...ids.filter((id) => !seen.includes(id))];
  }
  return order;
}

/**
 * @param {Record<string, unknown>} behavior
 */
export function overlaySettings(behavior) {
  const b = behavior ?? {};
  const list = Array.isArray(b.overlayMetrics) ? b.overlayMetrics : null;
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
    blocks: overlayVisibleBlocks(b.overlayBlocks, savedFields),
    fieldOrder: overlayFieldOrder(b.overlayFieldOrder),
    dock: DOCKS.includes(/** @type {any} */ (b.overlayDock))
      ? /** @type {typeof DOCKS[number]} */ (b.overlayDock)
      : "top",
    opacity: overlayOpacity(b.overlayOpacity),
    locked: b.overlayLocked === true,
    charts: chartMetrics(b),
    theme: typeof b.overlayTheme === "string" ? b.overlayTheme : "",
  };
}

/** @param {unknown} saved @param {MetricId[]} metrics */
export function overlayVisibleBlocks(saved, metrics) {
  if (Array.isArray(saved)) return BLOCK_IDS.filter((id) => saved.includes(id));
  const on = new Set(metrics);
  return BLOCKS.filter((block) => block.fields.some((id) => on.has(id))).map(
    (block) => block.id,
  );
}

/**
 * @param {string} id */
function blockOf(id) {
  return Object.values(BLOCK_OF).find((block) =>
    block.fields.includes(/** @type {MetricId} */ (id)),
  )?.id;
}

/**
 * @param {MetricId[]} metrics */
function derivedBlockOrder(metrics) {
  /** @type {string[]} */
  const seen = [];
  for (const id of metrics) {
    const block = blockOf(id);
    if (block && !seen.includes(block)) seen.push(block);
  }
  for (const block of BLOCK_IDS) {
    if (!seen.includes(block)) seen.push(block);
  }
  return seen;
}

/**
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

/**
 * @param {MetricId[]} metrics @param {string[]} order @param {MetricId[]} charts
 * @param {Record<string, MetricId[]>} [fieldOrder]
 * @param {Record<string, { id: string, target: string, after: boolean } | null>} [fieldMoves]
 * @param {string[]} [blocks] */
export function settingsBlocks(
  metrics,
  order,
  charts,
  fieldOrder = DEFAULT_FIELD_ORDER,
  fieldMoves = {},
  blocks = overlayVisibleBlocks(undefined, metrics),
) {
  const on = new Set(metrics);
  const visible = new Set(blocks);
  const charted = new Set(charts);
  return order
    .map((id) => BLOCK_OF[id])
    .filter(Boolean)
    .map((block) => {
      const single = block.fields.length === 1;
      const saved = fieldOrder?.[block.id] ?? block.fields;
      const preview = previewOrder(saved, fieldMoves?.[block.id] ?? null);
      const rows = preview.map((id) => ({
        id: /** @type {MetricId} */ (id),
        label: FIELD_LABELS[id] ?? `overlay.metric.${id}`,
        on: on.has(/** @type {MetricId} */ (id)),
      }));
      const field = block.fields[0];
      return {
        id: block.id,
        label: block.label,
        single,
        field: single ? field : null,
        on: visible.has(block.id),
        chartable: single ? chartable(field) : true,
        charted: charted.has(
          single ? field : /** @type {MetricId} */ (block.id),
        ),
        chartField: single ? null : /** @type {MetricId} */ (block.id),
        rows: single ? [] : rows,
      };
    });
}

/**
 * @param {MetricId | null | undefined} id */
function chartable(id) {
  return id != null && CHART_METRICS.includes(id);
}

/** @param {string[]} blocks @param {string} id @param {boolean} on */
export function toggleBlockVisibility(blocks, id, on) {
  if (!BLOCK_IDS.includes(id)) return blocks;
  const set = new Set(blocks);
  if (on) set.add(id);
  else set.delete(id);
  return BLOCK_IDS.filter((block) => set.has(block));
}

/**
 * @param {Record<string, { name: string, label: string }>} themes */
export function overlayThemeOptions(themes) {
  return Object.values(themes)
    .filter((t) => t.name !== "glassy")
    .map((t) => ({ value: t.name, label: t.label }));
}

/**
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

/**
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

/**
 * @param {Record<string, unknown>} b
 * @returns {MetricId[]} */
export function chartMetrics(b) {
  const list = b?.overlayChartMetrics;
  if (Array.isArray(list))
    return CHART_METRICS.filter((id) => list.includes(id));
  return b?.overlayCharts === true ? [...CHART_METRICS] : [];
}

/**
 * @param {MetricId[]} charts */
export function chartsPatch(charts) {
  return {
    overlayChartMetrics: CHART_METRICS.filter((id) => charts.includes(id)),
  };
}

/**
 * @param {MetricId[]} charts @param {MetricId} id @param {boolean} on */
export function toggleChart(charts, id, on) {
  if (!CHART_METRICS.includes(id)) return charts;
  const rest = charts.filter((m) => m !== id);
  return CHART_METRICS.filter((m) => (m === id ? on : rest.includes(m)));
}

/**
 * @param {MetricId[]} shown @param {MetricId} id @param {boolean} on */
export function toggleMetric(shown, id, on) {
  const rest = shown.filter((m) => m !== id);
  return on ? [...rest, id] : rest;
}

/**
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

/**
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
 *   gpuPower?: number|null,
 *   throttle?: {
 *     supported: boolean, thermal: boolean, thermalLog: boolean,
 *     prochot: boolean, prochotLog: boolean, package: boolean,
 *     distanceToTjMax: number|null,
 *     thermalPercent: number, packagePercent: number
 *   } | null,
 *   history?: any[]
 * }} Reading  (overlay_reading in overlay.rs)
 */

/**
 * @param {Reading | null} r @param {MetricId[]} shown
 * @param {string[]} [order] block order (defaults to the old flat order)
 * @param {Record<string, MetricId[]>} [fieldOrder]
 * @param {string[]} [blocks] visible blocks
 * @param {boolean} [throttleAfter] mark where the THR badge goes (CPU temp,
 *   or the CPU block's last field when temperature is off)
 * @returns {{ id: MetricId, key?: string, block?: string, value: string,
 *   parts: string[], label: string | null, throttleAfter?: true }[]} */
export function cells(
  r,
  shown,
  order = derivedBlockOrder(shown),
  fieldOrder = overlayFieldOrder(),
  blocks = overlayVisibleBlocks(undefined, shown),
  throttleAfter = false,
) {
  const x = r ?? {};
  const on = new Set(shown);
  const visible = new Set(blocks);
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
  /** @type {Partial<Record<MetricId, string>>} */
  const labels = {
    cpu: "",
    gpu: "",
    cpuPower: "",
    power: "",
    cpuTemp: "",
    gpuTemp: "",
    cpuClock: "CLK",
    cpuVoltage: voltLabel,
  };
  /** @type {{ id: MetricId, key?: string, block?: string, value: string,
   *   parts: string[], label: string | null, throttleAfter?: true }[]} */
  const out = [];
  for (const blockId of order) {
    const block = BLOCK_OF[blockId];
    if (!block || !visible.has(blockId)) continue;
    const fields = fieldOrder?.[blockId] ?? block.fields;
    const enabled = fields.filter((id) => on.has(id));
    if (!enabled.length) continue;
    let first = out.length;
    if (blockId === "cpu" || blockId === "gpu") {
      out.push({
        id: /** @type {MetricId} */ (blockId),
        key: `${blockId}:block`,
        block: blockId,
        value: "",
        parts: [],
        label: blockId === "cpu" ? "CPU" : "GPU",
      });
      first = out.length;
    }
    for (const id of enabled) {
      const parts = text[id]().split("	");
      out.push({
        id,
        value: parts.join(" "),
        parts,
        label: labels[id] ?? null,
      });
    }
    if (throttleAfter && blockId === "cpu") {
      const temp = out.findIndex(
        (cell, i) => i >= first && cell.id === "cpuTemp",
      );
      const target = temp >= 0 ? temp : out.length - 1;
      if (target >= first) out[target].throttleAfter = true;
    }
  }
  return out;
}

/**
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

/**
 * @param {MetricId} id @param {any[]} history */
export function chartFor(id, history) {
  const s = SERIES[id];
  if (!s) return null;
  const max =
    s.max ?? Math.max(1024, ...(history ?? []).map((x) => s.pick(x) ?? 0));
  return { pick: s.pick, max };
}
