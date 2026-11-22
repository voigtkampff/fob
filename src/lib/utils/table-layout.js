/** 5 px to spare for font differences on Windows (fitWidths). */
/** @type {Record<string, number>} */
const fitWidths = {
  pid: 72,
  ppid: 100,
  session_id: 100,
  status: 96,
  user: 130,
  cpu_usage: 80,
  gpu_usage: 80,
  memory_usage: 96,
  gpu_memory: 135,
  virtual_memory: 96,
  disk_read: 100,
  disk_write: 100,
  run_time: 120,
  start_time: 160,
};
/**
 * @type {Record<string, number>} */
const treeFitWidths = { cpu_usage: 105, memory_usage: 110 };
/**
 * @type {Record<string, [number, number]>} */
const elastic = {
  name: [180, 460],
  command: [200, 520],
  root: [200, 420],
};
/**
 * @type {[number, number]} */
const treeName = [280, 560];

export const actionsWidth = 112;
export const manualMin = 48;
export const manualMax = 1200;

/** @param {string} id @param {boolean} tree @returns {[number, number] | null} */
function bounds(id, tree) {
  if (id === "name") return tree ? treeName : elastic.name;
  return elastic[id] ?? null;
}

/**
 * @param {string} id @param {boolean} [tree] */
export function columnWidth(id, tree = false) {
  const range = bounds(id, tree);
  if (range) return range[0];
  return (
    (tree
      ? /** @type {Record<string, number>} */ (treeFitWidths)[id]
      : undefined) ??
    fitWidths[id] ??
    120
  );
}

/**
 * @param {unknown} saved @param {readonly string[]} ids
 * @returns {Record<string, number>} */
export function manualWidths(saved, ids) {
  if (!saved || typeof saved !== "object" || Array.isArray(saved)) return {};
  /** @type {Record<string, number>} */
  const kept = {};
  for (const [id, value] of Object.entries(saved))
    if (ids.includes(id) && typeof value === "number" && Number.isFinite(value))
      kept[id] = clampManual(value);
  return kept;
}

/** @param {number} width */
export const clampManual = (width) =>
  Math.round(Math.min(manualMax, Math.max(manualMin, width)));

/**
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} tree
 * @param {number} region @param {Record<string, number>} [manual]
 * @returns {{ widths: Record<string, number>, filler: number }} */
export function columnLayout(columns, tree, region, manual = {}) {
  const shown = columns.filter((c) => c.visible).map((c) => c.id);
  /** @type {Record<string, number>} */
  const widths = Object.fromEntries(
    shown.map((id) => [id, manual[id] ?? columnWidth(id, tree)]),
  );
  let spare = Math.floor(region - tableWidth(columns, tree, manual));
  for (const id of [
    "name",
    ...Object.keys(elastic).filter((id) => id !== "name"),
  ]) {
    const range = bounds(id, tree);
    if (spare <= 0 || !range || !(id in widths) || id in manual) continue;
    const grow = Math.min(range[1] - range[0], spare);
    widths[id] += grow;
    spare -= grow;
  }
  return { widths, filler: Math.max(0, spare) };
}

/**
 * @param {{id:string,visible:boolean}[]} columns @param {boolean} [tree]
 * @param {Record<string, number>} [manual] */
export function tableWidth(columns, tree = false, manual = {}) {
  return columns
    .filter((c) => c.visible)
    .reduce(
      (sum, c) => sum + (manual[c.id] ?? columnWidth(c.id, tree)),
      actionsWidth,
    );
}
