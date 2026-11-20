/** Process row context menu: the items, where the box lands, and which row a
 * pointer target belongs to. Kept free of Svelte so `node --test` can run it
 * directly; the icon names are looked up by the component. */

/** @typedef {import('../types').Process} Process */

/** @typedef {{
 *   id: string,
 *   labelKey: string,
 *   icon: string,
 *   disabled?: boolean,
 *   danger?: boolean,
 *   separator?: boolean,
 *   run: (process: Process, ctx: ProcMenuContext) => void,
 * }} ProcMenuItem */

/** @typedef {{
 *   onShowDetails: (process: Process) => void,
 *   onTogglePin: (command: string) => void,
 *   onKillProcess: (process: Process) => void,
 *   copyText: (text: string) => void,
 * }} ProcMenuContext */

/** @param {unknown} value */
const present = (value) => value != null && value !== "";

/**
 * The five row actions, in display order. An action whose data is missing is
 * marked disabled instead of dropped, so the menu keeps a stable shape.
 * @param {Process | null | undefined} process
 * @param {{pinned?: boolean}} [opts]
 * @returns {ProcMenuItem[]}
 */
export function procMenuItems(process, opts = {}) {
  const p = process ?? /** @type {any} */ ({});
  const pinned = opts.pinned === true;
  /** @type {ProcMenuItem[]} */
  const items = [
    {
      id: "details",
      labelKey: "proc.details",
      icon: "info",
      run: (proc, ctx) => ctx.onShowDetails(proc),
    },
    {
      id: "pin",
      labelKey: pinned ? "proc.unpin" : "proc.pin",
      icon: pinned ? "pinOff" : "pin",
      run: (proc, ctx) => ctx.onTogglePin(proc.command),
    },
    {
      id: "copyName",
      labelKey: "proc.copyName",
      icon: "copy",
      run: (proc, ctx) => ctx.copyText(proc.name),
    },
    {
      id: "copyPid",
      labelKey: "proc.copyPid",
      icon: "hash",
      run: (proc, ctx) => ctx.copyText(String(proc.pid)),
    },
    {
      id: "end",
      labelKey: "proc.end",
      icon: "x",
      danger: true,
      separator: true,
      run: (proc, ctx) => ctx.onKillProcess(proc),
    },
  ];
  /** @type {Record<string, boolean>} */
  const usable = {
    details: p.pid != null,
    pin: present(p.command),
    copyName: present(p.name),
    copyPid: p.pid != null,
    end: p.pid != null,
  };
  return items.map((item) =>
    usable[item.id] ? item : { ...item, disabled: true },
  );
}

/**
 * Place a menu box so it opens down-right of the pointer, flips to stay inside
 * the viewport on overflow, and never leaves it. Pure: sizes are arguments.
 * @param {{x: number, y: number}} anchor
 * @param {{width: number, height: number}} size
 * @param {{width: number, height: number}} viewport
 * @returns {{left: number, top: number}}
 */
export function menuPlacement(anchor, size, viewport) {
  const x = anchor?.x ?? 0;
  const y = anchor?.y ?? 0;
  const width = size?.width ?? 0;
  const height = size?.height ?? 0;
  const vw = viewport?.width ?? 0;
  const vh = viewport?.height ?? 0;
  /** @param {number} value @param {number} limit */
  const clamp = (value, limit) =>
    Math.max(0, Math.min(value, Math.max(0, limit)));
  return {
    left: clamp(x + width > vw ? x - width : x, vw - width),
    top: clamp(y + height > vh ? y - height : y, vh - height),
  };
}

/**
 * The process row a markup target sits in, for `closest` on any node kind.
 * @param {EventTarget | null | undefined} target
 * @returns {Element | null}
 */
export function procRowOf(target) {
  const el = /** @type {any} */ (target);
  if (!el || typeof el.closest !== "function") return null;
  return el.closest("[data-process-row]") ?? null;
}
