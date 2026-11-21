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

/**
 * @type {Record<string, string>}
 */
const locationHints = {
  checking: "proc.locationChecking",
  gone: "proc.gone",
  unreadable: "proc.locationUnreadable",
  failed: "proc.locationFailed",
};

/**
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
 * @param {number} requestId the ticket the lookup was issued with
 * @param {number} current the ticket of the menu that is open now
 * @returns {boolean}
 */
export function staleResponse(requestId, current) {
  return requestId !== current;
}

/**
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
 * @param {EventTarget | null | undefined} target
 * @returns {Element | null}
 */
export function procRowOf(target) {
  const el = /** @type {any} */ (target);
  if (!el || typeof el.closest !== "function") return null;
  return el.closest("[data-process-row]") ?? null;
}
