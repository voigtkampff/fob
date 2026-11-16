// Pointer reordering for the children of one element (table
// header cells, summary cards). A press becomes a drag after THRESHOLD px:
// the item lifts and follows the pointer, and the caller shows the order it
// would land in (onPreview) while neighbours slide into their new places.
// Release commits once (onMove); Escape or a cancelled pointer puts the
// order back. The click that ends a drag is swallowed so it neither sorts
// nor opens anything.
// Animations run only for a change in order, never while idle.
//
// A header cell lifts in place, because its column's body cells move with it
// inside the table's own scroll area. A card lifts as a copy in the top
// layer (a manual popover), so no scrolling or clipped ancestor can cut it
// off; the card itself stays in its slot, invisible, and keeps the drop
// order, focus and hit test.

import { tick } from "svelte";

const THRESHOLD = 6;
const SLIDE = { duration: 160, easing: "cubic-bezier(0.2, 0, 0, 1)" };

/** @typedef {{ id: string, target: string, after: boolean }} Move */
/**
 * @typedef {object} ReorderOptions
 * @property {(string | null)[]} ids one per child of the node; null children stay put and take no drop
 * @property {(id: string, target: string, after: boolean) => void} onMove commit
 * @property {(move: Move | null) => void} onPreview show the order a drag would land in; null shows the saved order
 * @property {string} [handle] a drag only starts on an element matching this selector (the title)
 * @property {boolean} [vertical] the items wrap into rows and follow the pointer up and down too
 * @property {(id: string) => HTMLElement[]} [cells] other elements that belong to an item (a column's body cells) and move with it; items without cells lift as a copy in the top layer
 * @property {boolean} [clampToNode] keep the lifted copy inside the visible box of `node` (a scrolling list)
 * @property {string} [clampToClosest] use the nearest ancestor matching this selector as the visible bounds
 * @property {boolean} [portal] append the lifted copy to document.body, outside any transformed or clipped ancestor
 * @property {boolean} [rejectOutside] no reorder while the pointer is outside the node's box
 * @property {boolean} [ignoreY] choose before or after from X instead of Y: a row of tall items drops on the side of the item the pointer is on, whatever its height
 */

/**
 * @typedef {{ left: number, top: number, right: number, bottom: number }} Box
 */

const reducedMotion = () =>
  matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Where an element's box is laid out, ignoring transforms and translate.
 * @param {HTMLElement} el */
function layoutBox(el) {
  const parent = /** @type {HTMLElement | null} */ (el.offsetParent);
  const origin = parent?.getBoundingClientRect() ?? { left: 0, top: 0 };
  let left =
    origin.left +
    el.offsetLeft +
    (parent?.clientLeft ?? 0) -
    (parent?.scrollLeft ?? 0);
  let top =
    origin.top +
    el.offsetTop +
    (parent?.clientTop ?? 0) -
    (parent?.scrollTop ?? 0);
  // A scroll container between the item and its offsetParent is not part of
  // offsetTop, so its scroll has to be removed by hand. Without this, items
  // in a scrolling settings pane report their unscrolled position and a
  // lifted row jumps away from the pointer.
  for (
    let node = el.parentElement;
    node && node !== parent;
    node = node.parentElement
  ) {
    left -= node.scrollLeft;
    top -= node.scrollTop;
  }
  return {
    left,
    top,
    right: left + el.offsetWidth,
    bottom: top + el.offsetHeight,
  };
}

/** The visible part of a node: its box intersected with every scrolling
 * ancestor. Used to keep a lifted row inside the list instead of letting it
 * cover the rest of the window.
 * @param {HTMLElement} el */
function visibleBox(el) {
  const rect = el.getBoundingClientRect();
  let box = {
    left: rect.left,
    top: rect.top,
    right: rect.right,
    bottom: rect.bottom,
  };
  for (
    let p = el.parentElement;
    p && p !== document.body;
    p = p.parentElement
  ) {
    const style = getComputedStyle(p);
    if (
      !/(auto|scroll)/.test(style.overflowY) &&
      !/(auto|scroll)/.test(style.overflowX)
    )
      continue;
    const r = p.getBoundingClientRect();
    box = {
      left: Math.max(box.left, r.left),
      top: Math.max(box.top, r.top),
      right: Math.min(box.right, r.right),
      bottom: Math.min(box.bottom, r.bottom),
    };
  }
  return box;
}

/** Animate elements from where they are drawn now (or from `drawn`) to
 * where `change` lays them out (FLIP).
 * @param {Element[]} elements @param {() => unknown} change
 * @param {Map<Element, DOMRect>} [drawn] */
async function slide(elements, change, drawn = new Map()) {
  const first = new Map(
    elements.map((el) => [el, drawn.get(el) ?? el.getBoundingClientRect()]),
  );
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

/** A copy of `el` drawn in the top layer over its slot. The copy sits in a
 * shallow copy of `list` placed next to it, so the selectors that style the
 * item (`.grid > [data-dragging]`, scoped classes) still match.
 * @param {HTMLElement} list @param {HTMLElement} el */
/** @param {HTMLElement} list @param {HTMLElement} el
 * @param {HTMLElement | null} [parent] */
function liftCopy(list, el, parent = null) {
  const layer = /** @type {HTMLElement} */ (list.cloneNode(false));
  const copy = /** @type {HTMLElement} */ (el.cloneNode(true));
  for (const e of [layer, copy, ...copy.querySelectorAll("[id]")])
    e.removeAttribute("id");
  layer.removeAttribute("aria-label");
  layer.setAttribute("aria-hidden", "true");
  layer.inert = true;
  layer.popover = "manual";
  Object.assign(layer.style, {
    position: "fixed",
    inset: "auto",
    margin: "0",
    padding: "0",
    border: "0",
    background: "none",
    color: "inherit",
    overflow: "visible",
    display: "block",
    width: `${el.offsetWidth}px`,
    height: `${el.offsetHeight}px`,
    pointerEvents: "none",
  });
  Object.assign(copy.style, {
    width: `${el.offsetWidth}px`,
    height: `${el.offsetHeight}px`,
    margin: "0",
  });
  copy.dataset.dragging = "";
  layer.append(copy);
  if (parent) parent.append(layer);
  else list.after(layer);
  // The popover top layer escapes scroll/overflow clipping; the portal is a
  // second guard against a transformed ancestor becoming the fixed
  // containing block.
  if ("showPopover" in HTMLElement.prototype) layer.showPopover();
  else layer.style.zIndex = "9999";
  return layer;
}

/** The slot a pointer asks for among laid-out items, as an index and a side.
 * The nearest item wins, measured down the page too when the items wrap into
 * rows, so a row change lands on the item under the pointer. The pointer's
 * side of that item then says before or after: from X when the items share a
 * row and from Y down a single column. `ignoreY` reads the side from X even
 * where the items differ only in Y, so a tall card does not need the pointer
 * past its vertical middle. Pure, so tests can pass plain boxes.
 * @param {{ x: number, y: number, items: Box[], from: number, vertical?: boolean, ignoreY?: boolean }} query
 * @returns {{ index: number, after: boolean } | null} */
export function dropSlot({ x, y, items, from, vertical, ignoreY }) {
  let best = null;
  let bestDistance = Infinity;
  for (const [index, box] of items.entries()) {
    const dx = Math.max(box.left - x, 0, x - box.right);
    const dy = vertical ? Math.max(box.top - y, 0, y - box.bottom) : 0;
    const distance = Math.hypot(dx, dy);
    if (distance < bestDistance) {
      bestDistance = distance;
      best = { index, box, rowMatch: dy === 0 };
    }
  }
  if (!best || best.index === from) return null;
  const past =
    ignoreY || (!vertical && best.rowMatch)
      ? x > (best.box.left + best.box.right) / 2
      : y > (best.box.top + best.box.bottom) / 2;
  if (best.index > from && past) return { index: best.index, after: true };
  if (best.index < from && !past) return { index: best.index, after: false };
  return null;
}

/** @param {HTMLElement} node @param {ReorderOptions} options */
export function dragReorder(node, options) {
  let o = options;
  /** @type {{ x: number, y: number, el: HTMLElement, id: string } | null} */
  let press = null;
  /** Set once the press has travelled THRESHOLD px. */
  let drag =
    /** @type {{ start: { left: number, top: number }, move: Move | null, busy: boolean, last: PointerEvent, layer: HTMLElement | null } | null} */ (
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

  /** Keep the lifted item under the pointer wherever its slot is now. A
   * copy in the top layer can be clamped to the list's visible box. */
  const follow = () => {
    if (!press || !drag) return;
    const slot = layoutBox(press.el);
    if (drag.layer) {
      const { offsetWidth: w, offsetHeight: h } = drag.layer;
      const boundsElement = o.clampToClosest
        ? /** @type {HTMLElement | null} */ (node.closest(o.clampToClosest))
        : node;
      const bounds =
        o.clampToNode || o.clampToClosest
          ? visibleBox(boundsElement ?? node)
          : { left: 0, top: 0, right: innerWidth, bottom: innerHeight };
      const x = drag.start.left + drag.last.clientX - press.x;
      const y = o.vertical
        ? drag.start.top + drag.last.clientY - press.y
        : slot.top;
      const fit = (
        /** @type {number} */ v,
        /** @type {number} */ min,
        /** @type {number} */ max,
      ) => Math.max(min, Math.min(v, Math.max(min, max)));
      drag.layer.style.left = `${fit(x, bounds.left, bounds.right - w)}px`;
      drag.layer.style.top = `${fit(y, bounds.top, bounds.bottom - h)}px`;
      return;
    }
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
    if (o.rejectOutside) {
      const boundsElement = o.clampToClosest
        ? /** @type {HTMLElement | null} */ (node.closest(o.clampToClosest))
        : node;
      const box = (boundsElement ?? node).getBoundingClientRect();
      if (x < box.left || x > box.right || y < box.top || y > box.bottom)
        return null;
    }
    const list = items();
    const from = list.findIndex((it) => it.id === press?.id);
    const slot = dropSlot({
      x,
      y,
      from,
      items: list.map((it) => layoutBox(it.el)),
      vertical: o.vertical,
      ignoreY: o.ignoreY,
    });
    if (!slot) return null;
    return { id: press.id, target: list[slot.index].id, after: slot.after };
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

  /** The scrolling ancestor a drag at the edge should scroll. */
  const scrollParent = () => {
    for (
      let el = node.parentElement;
      el && el !== document.body;
      el = el.parentElement
    ) {
      const style = getComputedStyle(el);
      if (
        /(auto|scroll)/.test(style.overflowY) &&
        el.scrollHeight > el.clientHeight
      )
        return el;
    }
    return null;
  };

  /** Keep scrolling while the pointer sits near the top or bottom edge. */
  let autoScrollFrame = 0;
  const autoScroll = () => {
    if (!drag) {
      autoScrollFrame = 0;
      return;
    }
    const box = scrollParent();
    if (box) {
      const rect = box.getBoundingClientRect();
      const y = drag.last.clientY;
      const edge = 32;
      let delta = 0;
      if (y < rect.top + edge) delta = -Math.ceil((rect.top + edge - y) / 5);
      else if (y > rect.bottom - edge)
        delta = Math.ceil((y - (rect.bottom - edge)) / 5);
      if (delta) {
        const before = box.scrollTop;
        box.scrollTop += delta;
        if (box.scrollTop !== before) {
          follow();
          void reflow();
        }
      }
    }
    autoScrollFrame = requestAnimationFrame(autoScroll);
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
    if (autoScrollFrame) {
      cancelAnimationFrame(autoScrollFrame);
      autoScrollFrame = 0;
    }
    if (!was || !it) return;
    swallowClick();
    delete document.documentElement.dataset.reordering;
    const lifted = group(it);
    // The card glides from where its copy was drawn into its final slot.
    const drawn = new Map(
      was.layer?.firstElementChild
        ? [[it.el, was.layer.firstElementChild.getBoundingClientRect()]]
        : [],
    );
    was.layer?.remove();
    void slide(
      everything(),
      () => {
        for (const el of lifted) {
          const h = /** @type {HTMLElement} */ (el);
          h.style.translate = "";
          delete h.dataset.dragging;
          delete h.dataset.dragSlot;
        }
        if (was.move && commit)
          o.onMove(was.move.id, was.move.target, was.move.after);
        if (was.move) o.onPreview(null);
      },
      drawn,
    );
  };
  const pointerMove = (/** @type {PointerEvent} */ e) => {
    if (!press) return;
    if (!drag) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < THRESHOLD)
        return;
      const box = layoutBox(press.el);
      // Without the popover API (an older WebView2) a card lifts in place.
      const layer = o.cells
        ? null
        : liftCopy(node, press.el, o.portal ? document.body : null);
      drag = { start: box, move: null, busy: false, last: e, layer };
      /** @type {HTMLElement} */ for (const el of group(press))
        if (layer) el.dataset.dragSlot = "";
        else el.dataset.dragging = "";
      document.documentElement.dataset.reordering = "";
      if (!autoScrollFrame) autoScrollFrame = requestAnimationFrame(autoScroll);
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
  node.addEventListener("pointerdown", down);
  return {
    /** @param {ReorderOptions} next */
    update(next) {
      o = next;
    },
    destroy() {
      end(false);
      node.removeEventListener("pointerdown", down);
    },
  };
}
