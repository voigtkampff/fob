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
 *   hintKey?: string,
 *   run: (process: Process, ctx: ProcMenuContext) => unknown,
 * }} ProcMenuItem */

/**
 * Where the executable lookup for the row stands: the reason the reveal item
 * is greyed out when it is not `ready`.
 * @typedef {{
 *   kind: "checking" | "ready" | "gone" | "unreadable" | "failed",
 * }} ProcMenuLocation
 */

/** @typedef {{
 *   onShowDetails: (process: Process) => void,
 *   onTogglePin: (command: string) => void,
 *   onKillProcess: (process: Process) => void,
 *   copyText: (text: string) => void,
 *   onOpenLocation: (process: Process) => unknown,
 * }} ProcMenuContext */

/**
 * The command's answer for one row: an executable, or why there is none.
 * @typedef {{
 *   kind: "found",
 *   path: string,
 * } | {
 *   kind: "gone",
 * } | {
 *   kind: "unreadable",
 * }} ProcLocationReply */

/** @param {unknown} value */
const present = (value) => value != null && value !== "";

/** Why the reveal item cannot be used, as an i18n key the component resolves.
 * `checking` is the fallback for a state this build does not know, so a greyed
 * reveal always says something.
 * @type {Record<string, string>}
 */
const locationHints = {
  checking: "proc.locationChecking",
  gone: "proc.gone",
  unreadable: "proc.locationUnreadable",
  failed: "proc.locationFailed",
};

/**
 * The six row actions, in display order. An action whose data is missing is
 * marked disabled instead of dropped, so the menu keeps a stable shape.
 * @param {Process | null | undefined} process
 * @param {{pinned?: boolean, location?: ProcMenuLocation}} [opts] `location`
 * defaults to `checking`, so an unresolved lookup never enables the reveal.
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
      id: "location",
      labelKey: "proc.location",
      icon: "folderOpen",
      run: (proc, ctx) => ctx.onOpenLocation(proc),
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
  const kind = opts.location?.kind ?? "checking";
  /** @type {Record<string, boolean>} */
  const usable = {
    details: p.pid != null,
    pin: present(p.command),
    copyName: present(p.name),
    copyPid: p.pid != null,
    location: kind === "ready",
    end: p.pid != null,
  };
  return items.map((item) => {
    if (usable[item.id]) return item;
    const hintKey =
      item.id === "location"
        ? (locationHints[kind] ?? locationHints.checking)
        : undefined;
    return hintKey
      ? { ...item, disabled: true, hintKey }
      : { ...item, disabled: true };
  });
}

/**
 * What a lookup's reply does to the open menu: the path once it is known, or
 * the reason the reveal stays greyed out. A reply this build does not know
 * reads as unreadable instead of enabling the item on a guess.
 * @param {ProcLocationReply | null | undefined} reply
 * @returns {{location: ProcMenuLocation, path: string}}
 */
export function locateOutcome(reply) {
  if (reply?.kind === "found" && present(reply.path))
    return { location: { kind: "ready" }, path: reply.path };
  return {
    location: { kind: reply?.kind === "gone" ? "gone" : "unreadable" },
    path: "",
  };
}

/**
 * Whether a lookup's reply arrived too late: its menu was closed or replaced by
 * another row's, so the answer must not be written back.
 * @param {number} requestId the ticket the lookup was issued with
 * @param {number} current the ticket of the menu that is open now
 * @returns {boolean}
 */
export function staleResponse(requestId, current) {
  return requestId !== current;
}

/**
 * The reveal after the click. No path never asks Explorer, and a refusal keeps
 * the menu open (`keepOpen`) so its item can say why instead of vanishing.
 * @param {{path?: string} | null | undefined} menu
 * @param {(path: string) => Promise<unknown>} reveal
 * @returns {Promise<{keepOpen: boolean, location?: ProcMenuLocation}>}
 */
export async function revealLocation(menu, reveal) {
  const path = menu?.path ?? "";
  if (!present(path)) return { keepOpen: true, location: { kind: "failed" } };
  try {
    await reveal(path);
    return { keepOpen: false };
  } catch {
    return { keepOpen: true, location: { kind: "failed" } };
  }
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
