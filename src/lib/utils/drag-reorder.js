// Pointer and keyboard reordering for the children of one element (table
// header cells, summary cards). Nothing moves until the pointer travels
// THRESHOLD px, so a short press stays a click; after a drag, or Escape
// during one, the click that follows is swallowed so it neither sorts nor
// opens anything. Alt+Arrow moves the focused item one step. The order
// itself lives in the caller's state; this only reports the requested move.

import { tick } from "svelte";

const THRESHOLD = 6;

/**
 * @typedef {object} ReorderOptions
 * @property {(string | null)[]} ids one per child of the node; null children stay put and take no drop
 * @property {(id: string, target: string, after: boolean) => void} onMove
 * @property {(id: string, step: -1 | 1) => void} onStep
 * @property {string} [handle] a drag only starts on an element matching this selector
 * @property {boolean} [vertical] Up and Down step too (wrapping grids)
 */

/** @param {HTMLElement} node @param {ReorderOptions} options */
export function dragReorder(node, options) {
  let o = options;
  /** @type {{ x: number, y: number, el: HTMLElement, id: string } | null} */
  let press = null;
  let dragging = false;
  /** @type {{ el: HTMLElement, id: string, after: boolean } | null} */
  let drop = null;

  const items = () =>
    /** @type {HTMLElement[]} */ ([...node.children]).flatMap((el, i) =>
      o.ids[i] ? [{ el, id: /** @type {string} */ (o.ids[i]) }] : [],
    );
  /** @param {EventTarget | null} target */
  const itemAt = (target) =>
    target instanceof Node
      ? items().find((it) => it.el.contains(target))
      : undefined;

  const mark = (/** @type {typeof drop} */ next) => {
    if (drop && (drop.el !== next?.el || drop.after !== next?.after))
      delete drop.el.dataset.drop;
    drop = next;
    if (drop) drop.el.dataset.drop = drop.after ? "after" : "before";
  };
  /** The item under the pointer, or the nearest one, and which side. */
  const target = (/** @type {PointerEvent} */ e) => {
    if (!press) return null;
    let best = null;
    let bestDistance = Infinity;
    for (const it of items()) {
      const r = it.el.getBoundingClientRect();
      const dx = Math.max(r.left - e.clientX, 0, e.clientX - r.right);
      const dy = Math.max(r.top - e.clientY, 0, e.clientY - r.bottom);
      const distance = Math.hypot(dx, dy);
      if (distance < bestDistance) {
        bestDistance = distance;
        best = { ...it, after: e.clientX > r.left + r.width / 2 };
      }
    }
    if (!best || best.id === press.id) return null;
    // Dropping on the near side of a neighbour changes nothing.
    const list = items();
    const from = list.findIndex((it) => it.id === press?.id);
    const to = list.findIndex((it) => it.id === best.id);
    if ((best.after && to === from - 1) || (!best.after && to === from + 1))
      return null;
    return best;
  };

  const swallowClick = () => {
    const stop = (/** @type {Event} */ e) => {
      e.preventDefault();
      e.stopPropagation();
    };
    node.addEventListener("click", stop, { capture: true, once: true });
    // A click, if one comes, is dispatched before this runs.
    setTimeout(() => node.removeEventListener("click", stop, true), 0);
  };
  const end = (/** @type {boolean} */ commit) => {
    if (commit && press && drop) o.onMove(press.id, drop.id, drop.after);
    if (dragging) swallowClick();
    mark(null);
    if (press) delete press.el.dataset.dragging;
    delete document.documentElement.dataset.reordering;
    press = null;
    dragging = false;
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", cancel);
    window.removeEventListener("keydown", escape, true);
  };
  const move = (/** @type {PointerEvent} */ e) => {
    if (!press) return;
    if (!dragging) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < THRESHOLD)
        return;
      dragging = true;
      press.el.dataset.dragging = "";
      document.documentElement.dataset.reordering = "";
    }
    mark(target(e));
  };
  const up = () => end(true);
  const cancel = () => end(false);
  const escape = (/** @type {KeyboardEvent} */ e) => {
    if (e.key !== "Escape") return;
    e.preventDefault();
    e.stopPropagation();
    end(false);
    // The button is still down; the click its release makes is cancelled too.
    window.addEventListener("pointerup", swallowClick, {
      capture: true,
      once: true,
    });
  };
  const down = (/** @type {PointerEvent} */ e) => {
    if (e.button !== 0 || press) return;
    const it = itemAt(e.target);
    if (!it) return;
    if (o.handle && !(/** @type {Element} */ (e.target).closest(o.handle)))
      return;
    press = { x: e.clientX, y: e.clientY, ...it };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", escape, true);
  };
  const key = (/** @type {KeyboardEvent} */ e) => {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    const step =
      e.key === "ArrowRight" || (o.vertical && e.key === "ArrowDown")
        ? 1
        : e.key === "ArrowLeft" || (o.vertical && e.key === "ArrowUp")
          ? -1
          : 0;
    const it = itemAt(e.target);
    if (!step || !it) return;
    e.preventDefault();
    // Moving a keyed element in the DOM drops its focus; give it back.
    const focused = document.activeElement;
    o.onStep(it.id, /** @type {-1 | 1} */ (step));
    void tick().then(() => {
      if (focused instanceof HTMLElement && focused !== document.activeElement)
        focused.focus();
    });
  };
  node.addEventListener("pointerdown", down);
  node.addEventListener("keydown", key);
  return {
    /** @param {ReorderOptions} next */
    update(next) {
      o = next;
    },
    destroy() {
      end(false);
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("keydown", key);
    },
  };
}
