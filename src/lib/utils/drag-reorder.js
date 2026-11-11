// Pointer and keyboard reordering for the children of one element (table
// header cells, summary cards). A press becomes a drag after THRESHOLD px:
// the item lifts and follows the pointer, and the caller shows the order it
// would land in (onPreview) while neighbours slide into their new places.
// Release commits once (onMove); Escape or a cancelled pointer puts the
// order back. The click that ends a drag is swallowed so it neither sorts
// nor opens anything. Alt+Arrow moves the focused item one place.
// Animations run only for a change in order, never while idle.

import { tick } from "svelte";

const THRESHOLD = 6;
const SLIDE = { duration: 160, easing: "cubic-bezier(0.2, 0, 0, 1)" };

/** @typedef {{ id: string, target: string, after: boolean }} Move */
/**
 * @typedef {object} ReorderOptions
 * @property {(string | null)[]} ids one per child of the node; null children stay put and take no drop
 * @property {(id: string, target: string, after: boolean) => void} onMove commit
 * @property {(move: Move | null) => void} onPreview show the order a drag would land in; null shows the saved order
 * @property {(id: string, step: -1 | 1) => void} onStep
 * @property {string} [handle] a drag only starts on an element matching this selector
 * @property {boolean} [vertical] the items wrap into rows; Up and Down step too
 * @property {(id: string) => HTMLElement[]} [cells] other elements that belong to an item (a column's body cells) and move with it
 */

const reducedMotion = () =>
  matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Where an element's box is laid out, ignoring transforms and translate.
 * @param {HTMLElement} el */
function layoutBox(el) {
  const parent = /** @type {HTMLElement | null} */ (el.offsetParent);
  const origin = parent?.getBoundingClientRect() ?? { left: 0, top: 0 };
  const left =
    origin.left +
    el.offsetLeft +
    (parent?.clientLeft ?? 0) -
    (parent?.scrollLeft ?? 0);
  const top =
    origin.top +
    el.offsetTop +
    (parent?.clientTop ?? 0) -
    (parent?.scrollTop ?? 0);
  return {
    left,
    top,
    right: left + el.offsetWidth,
    bottom: top + el.offsetHeight,
  };
}

/** Animate elements from where they are drawn now to where `change` lays
 * them out (FLIP).
 * @param {Element[]} elements @param {() => unknown} change */
async function slide(elements, change) {
  const first = new Map(elements.map((el) => [el, el.getBoundingClientRect()]));
  change();
  await tick();
  if (reducedMotion()) return;
  for (const [el, from] of first) {
    if (!el.isConnected) continue;
    for (const a of el.getAnimations()) if (a.id === "reorder") a.cancel();
    const to = el.getBoundingClientRect();
    const dx = from.left - to.left;
    const dy = from.top - to.top;
    if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue;
    const a = el.animate(
      [{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }],
      SLIDE,
    );
    a.id = "reorder";
  }
}

/** @param {HTMLElement} node @param {ReorderOptions} options */
export function dragReorder(node, options) {
  let o = options;
  /** @type {{ x: number, y: number, el: HTMLElement, id: string } | null} */
  let press = null;
  /** Set once the press has travelled THRESHOLD px. */
  let drag =
    /** @type {{ start: { left: number, top: number }, move: Move | null, busy: boolean, last: PointerEvent } | null} */ (
      null
    );

  const items = () =>
    /** @type {HTMLElement[]} */ ([...node.children]).flatMap((el, i) =>
      o.ids[i] ? [{ el, id: /** @type {string} */ (o.ids[i]) }] : [],
    );
  /** @param {EventTarget | null} target */
  const itemAt = (target) =>
    target instanceof Node
      ? items().find((it) => it.el.contains(target))
      : undefined;
  /** An item and the elements that move with it.
   * @param {{ el: HTMLElement, id: string }} it @returns {HTMLElement[]} */
  const group = (it) => [it.el, ...(o.cells?.(it.id) ?? [])];
  const everything = () => items().flatMap(group);

  /** Keep the lifted item under the pointer wherever its slot is now. */
  const follow = () => {
    if (!press || !drag) return;
    const slot = layoutBox(press.el);
    const dx = drag.last.clientX - press.x - (slot.left - drag.start.left);
    const dy = o.vertical
      ? drag.last.clientY - press.y - (slot.top - drag.start.top)
      : 0;
    /** @type {HTMLElement} */ for (const el of group(press))
      el.style.translate = `${dx}px ${dy}px`;
  };

  /** The move the pointer asks for: past the middle of a later item, or
   * before the middle of an earlier one. */
  const wanted = () => {
    if (!press || !drag) return null;
    const { clientX: x, clientY: y } = drag.last;
    const list = items();
    const from = list.findIndex((it) => it.id === press?.id);
    let best = null;
    let bestDistance = Infinity;
    for (const [i, it] of list.entries()) {
      const r = layoutBox(it.el);
      const dx = Math.max(r.left - x, 0, x - r.right);
      const dy = o.vertical ? Math.max(r.top - y, 0, y - r.bottom) : 0;
      const distance = Math.hypot(dx, dy);
      if (distance < bestDistance) {
        bestDistance = distance;
        best = { i, it, r, rowMatch: dy === 0 };
      }
    }
    if (!best || best.i === from) return null;
    const { r } = best;
    const past = best.rowMatch
      ? x > (r.left + r.right) / 2
      : y > (r.top + r.bottom) / 2;
    if (best.i > from && past)
      return { id: press.id, target: best.it.id, after: true };
    if (best.i < from && !past)
      return { id: press.id, target: best.it.id, after: false };
    return null;
  };

  const reflow = async () => {
    if (!drag || drag.busy) return;
    const move = wanted();
    if (!move) return follow();
    drag.busy = true;
    drag.move = move;
    const lifted = press ? group(press) : [];
    await slide(
      everything().filter((el) => !lifted.includes(el)),
      () => o.onPreview(move),
    );
    if (!drag) return;
    drag.busy = false;
    follow();
    void reflow();
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
    window.removeEventListener("pointermove", pointerMove);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", cancel);
    window.removeEventListener("keydown", escape, true);
    const was = drag;
    const it = press;
    press = null;
    drag = null;
    if (!was || !it) return;
    swallowClick();
    delete document.documentElement.dataset.reordering;
    const lifted = group(it);
    void slide(everything(), () => {
      for (const el of lifted) {
        const h = /** @type {HTMLElement} */ (el);
        h.style.translate = "";
        delete h.dataset.dragging;
      }
      if (was.move && commit)
        o.onMove(was.move.id, was.move.target, was.move.after);
      if (was.move) o.onPreview(null);
    });
  };
  const pointerMove = (/** @type {PointerEvent} */ e) => {
    if (!press) return;
    if (!drag) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < THRESHOLD)
        return;
      const box = layoutBox(press.el);
      drag = { start: box, move: null, busy: false, last: e };
      /** @type {HTMLElement} */ for (const el of group(press))
        el.dataset.dragging = "";
      document.documentElement.dataset.reordering = "";
    }
    drag.last = e;
    follow();
    void reflow();
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
    window.addEventListener("pointermove", pointerMove);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", escape, true);
  };
  const key = (/** @type {KeyboardEvent} */ e) => {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey || press) return;
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
    void slide(everything(), () =>
      o.onStep(it.id, /** @type {-1 | 1} */ (step)),
    ).then(() => {
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
