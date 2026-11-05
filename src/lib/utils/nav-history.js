// Fob: Explorer-style back/forward over the folders of the space map.

/**
 * @typedef {{ back: string[][], forward: string[][] }} History
 *  Folders (names below the drive root) left behind, most recent last.
 */

/** @returns {History} */
export const emptyHistory = () => ({ back: [], forward: [] });

const same = (/** @type {string[]} */ a, /** @type {string[]} */ b) =>
  a.length === b.length && a.every((n, i) => n === b[i]);

/** Going from `from` to a new folder `to`: `from` joins the back stack and
 * the forward stack is dropped. Going nowhere changes nothing.
 * @param {History} h @param {string[]} from @param {string[]} to
 * @returns {History} */
export function visit(h, from, to) {
  if (same(from, to)) return h;
  return { back: [...h.back, from], forward: [] };
}

/** One step back (`dir` -1) or forward (+1) from `current`; null when that
 * stack is empty.
 * @param {History} h @param {string[]} current @param {-1 | 1} dir
 * @returns {{ history: History, path: string[] } | null} */
export function step(h, current, dir) {
  const from = dir < 0 ? h.back : h.forward;
  if (!from.length) return null;
  const path = /** @type {string[]} */ (from.at(-1));
  const rest = from.slice(0, -1);
  return dir < 0
    ? { path, history: { back: rest, forward: [...h.forward, current] } }
    : { path, history: { back: [...h.back, current], forward: rest } };
}
