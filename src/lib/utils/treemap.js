// Fob: disk space map layout — squarified treemap (Bruijn, Huizing & van
// Wijk) and the folding of small items into one "other small files" block.

/**
 * @template T
 * @param {T[]} items
 * @param {(item: T) => number} value
 * @param {{ x: number, y: number, w: number, h: number }} rect
 * @returns {{ item: T, x: number, y: number, w: number, h: number }[]}
 *  One rectangle per item with a positive value, largest first; the areas
 *  are proportional to the values and together fill `rect`.
 */
export function squarify(items, value, rect) {
  const sized = items
    .map((item) => ({ item, v: value(item) }))
    .filter((n) => Number.isFinite(n.v) && n.v > 0)
    .sort((a, b) => b.v - a.v);
  const total = sized.reduce((sum, n) => sum + n.v, 0);
  if (!total || rect.w <= 0 || rect.h <= 0) return [];
  const scale = (rect.w * rect.h) / total;
  const nodes = sized.map((n) => ({ item: n.item, area: n.v * scale }));
  /** @type {{ item: T, x: number, y: number, w: number, h: number }[]} */
  const out = [];
  let { x, y, w, h } = rect;
  /** Worst aspect ratio of a row laid along a side of length `side`. */
  const worst = (
    /** @type {typeof nodes} */ row,
    /** @type {number} */ side,
  ) => {
    const s = row.reduce((sum, n) => sum + n.area, 0);
    const max = Math.max(...row.map((n) => n.area));
    const min = Math.min(...row.map((n) => n.area));
    return Math.max(
      (side * side * max) / (s * s),
      (s * s) / (side * side * min),
    );
  };
  const place = (/** @type {typeof nodes} */ row, last = false) => {
    const s = row.reduce((sum, n) => sum + n.area, 0);
    if (w >= h) {
      // A column on the left.
      const t = last ? w : s / h;
      let cy = y;
      row.forEach((n, i) => {
        const nh = i === row.length - 1 ? y + h - cy : n.area / t;
        out.push({ item: n.item, x, y: cy, w: t, h: nh });
        cy += nh;
      });
      x += t;
      w -= t;
    } else {
      // A row along the top.
      const t = last ? h : s / w;
      let cx = x;
      row.forEach((n, i) => {
        const nw = i === row.length - 1 ? x + w - cx : n.area / t;
        out.push({ item: n.item, x: cx, y, w: nw, h: t });
        cx += nw;
      });
      y += t;
      h -= t;
    }
  };
  /** @type {typeof nodes} */
  let row = [];
  for (let i = 0; i < nodes.length; ) {
    const side = Math.min(w, h);
    const candidate = [...row, nodes[i]];
    if (!row.length || worst(candidate, side) <= worst(row, side)) {
      row = candidate;
      i++;
    } else {
      place(row);
      row = [];
    }
  }
  if (row.length) place(row, true);
  return out;
}

/**
 * @typedef {{ n: string, s: number, f: number, d?: boolean, c?: SpaceNode[],
 *   o?: number, oc?: number, x?: boolean }} SpaceNode
 *  A folder or file of a scan (see src-tauri/src/space.rs).
 * @typedef {{ node: SpaceNode | null, size: number, count: number }} Block
 *  `node` null: the folded "other small files" block of `count` items.
 */

/**
 * The blocks of one folder: its largest items, then one block for the rest
 * (items under `minShare` of the folder, past `maxItems`, and what the scan
 * already folded).
 * @param {SpaceNode} folder
 * @param {{ minShare?: number, maxItems?: number }} [options]
 * @returns {Block[]}
 */
export function blocks(folder, { minShare = 0.004, maxItems = 60 } = {}) {
  const kids = [...(folder.c ?? [])].sort((a, b) => b.s - a.s);
  /** @type {Block[]} */
  const out = [];
  let rest = folder.o ?? 0;
  let count = folder.oc ?? 0;
  for (const kid of kids) {
    if (out.length < maxItems && folder.s > 0 && kid.s / folder.s >= minShare)
      out.push({ node: kid, size: kid.s, count: 1 });
    else {
      rest += kid.s;
      count += 1;
    }
  }
  if (rest > 0) out.push({ node: null, size: rest, count });
  return out;
}

/** The folder at `path` (names below the root), or null if it is gone.
 * @param {SpaceNode} root @param {string[]} path */
export function folderAt(root, path) {
  let node = root;
  for (const name of path) {
    const next = node.c?.find((c) => c.n === name && c.d);
    if (!next) return null;
    node = next;
  }
  return node;
}

/** Full path of `names` below `root`. @param {string} root
 * @param {string[]} names @param {string} sep */
export function joinPath(root, names, sep) {
  if (!names.length) return root;
  return (root.endsWith(sep) ? root : root + sep) + names.join(sep);
}

const TYPES = /** @type {const} */ ({
  video: "mp4 mkv avi mov wmv webm m4v flv ts mpg",
  media:
    "jpg jpeg png gif webp heic bmp tif tiff raw psd mp3 flac wav aac ogg m4a",
  archive:
    "zip 7z rar tar gz tgz xz bz2 zst iso img vhd vhdx vmdk wim esd cab msi",
  program: "exe dll sys pak bin dat db sqlite node so dylib lib obj pdb jar",
  document:
    "pdf doc docx xls xlsx ppt pptx txt md csv json xml js ts rs py c h cpp log",
});
/** @type {Map<string, keyof typeof TYPES>} */
const BY_EXTENSION = new Map(
  Object.entries(TYPES).flatMap(([kind, list]) =>
    list
      .split(" ")
      .map((ext) => [ext, /** @type {keyof typeof TYPES} */ (kind)]),
  ),
);
/** File type of a file name, for its colour. @param {string} name */
export function fileKind(name) {
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return "other";
  return BY_EXTENSION.get(name.slice(dot + 1).toLowerCase()) ?? "other";
}

/** Theme colour of each file type (folders are neutral boxes). */
export const KIND_TONE = {
  video: "var(--status-stopped)",
  media: "var(--chart-2)",
  archive: "var(--chart-3)",
  program: "var(--chart-4)",
  document: "var(--chart-5)",
  other: "var(--chart-1)",
};
/** Background of a block: a file's type colour, or a folder's depth shade,
 * mixed into the card colour so the theme's text stays readable on it.
 * @param {keyof typeof KIND_TONE | "folder" | "rest"} kind
 * @param {1 | 2} [depth] */
export function tileColor(kind, depth = 1) {
  if (kind === "folder")
    return `color-mix(in oklab, var(--foreground) ${depth === 1 ? 6 : 16}%, var(--card))`;
  if (kind === "rest")
    return "repeating-linear-gradient(135deg, color-mix(in oklab, var(--muted-foreground) 18%, var(--card)) 0 6px, color-mix(in oklab, var(--muted-foreground) 28%, var(--card)) 6px 12px)";
  return `color-mix(in oklab, ${KIND_TONE[kind]} ${kind === "other" ? 34 : 45}%, var(--card))`;
}
