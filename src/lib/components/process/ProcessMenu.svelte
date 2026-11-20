<script lang="ts">
  import type { Process } from "$lib/types";
  import type {
    ProcMenuContext,
    ProcMenuItem,
  } from "$lib/utils/process-menu.js";
  import { menuPlacement } from "$lib/utils/process-menu.js";
  import { t } from "$lib/i18n";
  import { Copy, FolderOpen, Hash, Info, Pin, PinOff, X } from "@lucide/svelte";
  import { onMount } from "svelte";

  export let items: ProcMenuItem[] = [];
  export let process: Process;
  export let anchor: { x: number; y: number; row: HTMLElement | null };
  export let ctx: ProcMenuContext;
  export let onClose: () => void = () => {};

  const icons = {
    info: Info,
    pin: Pin,
    pinOff: PinOff,
    copy: Copy,
    hash: Hash,
    folderOpen: FolderOpen,
    x: X,
  };
  const iconFor = (name: string) => icons[name as keyof typeof icons] ?? Info;

  let el: HTMLDivElement | null = null;
  let left = 0;
  let top = 0;

  function place() {
    if (!el) return;
    const size = { width: el.offsetWidth, height: el.offsetHeight };
    const viewport = { width: window.innerWidth, height: window.innerHeight };
    ({ left, top } = menuPlacement(anchor, size, viewport));
  }

  // The list sits in a scrolling container, so the box has to hang off body:
  // otherwise the table's scroll offset would drag it along.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    el = node as HTMLDivElement;
    place();
    entries()[0]?.focus();
    return {
      destroy() {
        node.remove();
      },
    };
  }

  /** Buttons a key can land on: disabled ones are not menu items. */
  function entries() {
    if (!el) return [];
    return [
      ...el.querySelectorAll<HTMLButtonElement>("button[role=menuitem]"),
    ].filter((button) => !button.disabled);
  }

  function move(step: 1 | -1) {
    const list = entries();
    if (!list.length) return;
    const at = list.indexOf(document.activeElement as HTMLButtonElement);
    const from = at < 0 ? (step > 0 ? -1 : 0) : at;
    list[(from + step + list.length) % list.length]?.focus();
  }

  function back() {
    anchor?.row?.focus();
    onClose();
  }

  function keydown(event: KeyboardEvent) {
    const list = entries();
    const at = list.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "ArrowDown") {
      event.preventDefault();
      move(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      move(-1);
    } else if (event.key === "Home") {
      event.preventDefault();
      list[0]?.focus();
    } else if (event.key === "End") {
      event.preventDefault();
      list[list.length - 1]?.focus();
    } else if (event.key === "Tab") {
      // Tab walks the items and then leaves the menu instead of wrapping.
      const next = at + (event.shiftKey ? -1 : 1);
      if (next < 0 || next >= list.length) return;
      event.preventDefault();
      list[next]?.focus();
    } else if (event.key === "Escape") {
      event.preventDefault();
      back();
    }
    // Enter and Space are the buttons' own activation.
  }

  $: if (el && anchor) place();

  onMount(() => {
    const outside = (event: PointerEvent) => {
      if (el && !el.contains(event.target as Node)) onClose();
    };
    // A field's scroll is the table's; the box would keep a stale position.
    const scrolled = (event: Event) => {
      const target = event.target as Node | null;
      if (target instanceof Element && target.closest(".process-table-region"))
        onClose();
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("scroll", scrolled, true);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("scroll", scrolled, true);
      window.removeEventListener("resize", onClose);
    };
  });
</script>

<div
  bind:this={el}
  use:portal
  class="menu"
  role="menu"
  aria-label={$t("proc.menuAria")}
  tabindex="-1"
  style:left={`${left}px`}
  style:top={`${top}px`}
  onkeydown={keydown}
>
  {#each items as item (item.id)}
    {@const Icon = iconFor(item.icon)}
    {#if item.separator}<div class="sep" role="separator"></div>{/if}<button
      type="button"
      role="menuitem"
      class="item"
      class:danger={item.danger}
      disabled={item.disabled}
      onclick={() => {
        // A reveal can fail after the click: the menu then stays open, its
        // item showing why.
        const run = item.run(process, ctx);
        if (run instanceof Promise)
          void run.then((keep) => keep !== false && onClose());
        else onClose();
      }}
      ><Icon size={14} /><span class="label"
        ><span>{$t(item.labelKey)}</span>{#if item.hintKey}<span class="hint"
            >{$t(item.hintKey)}</span
          >{/if}</span
      ></button
    >
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 50;
    display: flex;
    flex-direction: column;
    min-width: 168px;
    padding: 4px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--popover);
    color: var(--popover-foreground);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.2);
    font-size: 12.5px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    border-radius: 4px;
    text-align: left;
    white-space: nowrap;
  }
  /* :focus, not :focus-visible: a right-click opens the menu and the first
     item takes focus, which Chromium would not paint as focus-visible. */
  .item:hover:not(:disabled),
  .item:focus {
    background: var(--accent);
  }
  .label {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .hint {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .item:disabled {
    opacity: 0.45;
  }
  .item.danger {
    color: var(--destructive);
  }
  .item.danger:hover:not(:disabled),
  .item.danger:focus {
    background: color-mix(in oklab, var(--destructive) 14%, transparent);
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
</style>
