<script lang="ts">
  // Fob: the squarified treemap of one folder. Folders big enough show their
  // own largest items inside; clicking a folder opens it, right-click offers
  // File Explorer. Blocks are absolutely positioned divs (no canvas), so
  // labels use the theme's text colours in light and dark themes.
  import {
    squarify,
    blocks,
    joinPath,
    fileKind,
    tileColor,
    type SpaceNode,
    type Block,
  } from "$lib/utils/treemap.js";
  import { tooltipLeft } from "$lib/utils/chart-cursor.js";
  import { formatBytes } from "$lib/utils";
  import { t } from "$lib/i18n";
  let {
    folder,
    root,
    path,
    sep,
    onOpen,
    onMenu,
  }: {
    folder: SpaceNode;
    root: string;
    path: string[];
    sep: string;
    /** Open the folder at these names (below the root). */
    onOpen: (names: string[]) => void;
    /** Right-click: the item's full path at the pointer. */
    onMenu: (target: string, x: number, y: number) => void;
  } = $props();

  type Tile = {
    key: string;
    block: Block;
    parent: SpaceNode;
    names: string[];
    depth: 1 | 2;
    x: number;
    y: number;
    w: number;
    h: number;
  };
  const HEADER = 18;
  const PAD = 3;
  let width = $state(0);
  let height = $state(0);
  const tiles = $derived.by(() => {
    const out: Tile[] = [];
    const outer = squarify(blocks(folder, { maxItems: 250 }), (b) => b.size, {
      x: 0,
      y: 0,
      w: width,
      h: height,
    });
    for (const r of outer) {
      const names = r.item.node ? [...path, r.item.node.n] : path;
      out.push({
        key: names.join("/") + (r.item.node ? "" : "/*"),
        block: r.item,
        parent: folder,
        names,
        depth: 1,
        ...r,
      });
      const inner = r.item.node;
      if (inner?.d && r.w >= 70 && r.h >= 44)
        for (const c of squarify(
          blocks(inner, { minShare: 0.01, maxItems: 60 }),
          (b) => b.size,
          {
            x: r.x + PAD,
            y: r.y + HEADER,
            w: r.w - 2 * PAD,
            h: r.h - HEADER - PAD,
          },
        ))
          out.push({
            key: [...names, c.item.node?.n ?? "*"].join("/"),
            block: c.item,
            parent: inner,
            names: c.item.node ? [...names, c.item.node.n] : names,
            depth: 2,
            ...c,
          });
    }
    return out;
  });
  const color = (tile: Tile) => {
    const node = tile.block.node;
    if (!node) return tileColor("rest");
    return node.d
      ? tileColor("folder", tile.depth)
      : tileColor(fileKind(node.n));
  };
  const label = (tile: Tile) =>
    tile.block.node
      ? tile.block.node.n
      : $t("space.other", { n: tile.block.count });

  // Hover tooltip: full path, size, share of its folder.
  let hover = $state<{ tile: Tile; x: number; y: number } | null>(null);
  let tipWidth = $state(0);
  let tipHeight = $state(0);
  // The "other" block's names are its folder's.
  const fullPath = (tile: Tile) => joinPath(root, tile.names, sep);
  function pick(event: MouseEvent): Tile | null {
    const el = (event.target as HTMLElement).closest("[data-tile]");
    return el ? (tiles[Number(el.getAttribute("data-tile"))] ?? null) : null;
  }
  function move(event: MouseEvent) {
    const tile = pick(event);
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    hover = tile
      ? { tile, x: event.clientX - box.left, y: event.clientY - box.top }
      : null;
  }
  function click(event: MouseEvent) {
    const tile = pick(event);
    if (!tile) return;
    // A folder opens; a file or the "other" block opens the folder it is in.
    const node = tile.block.node;
    const names = node?.d
      ? tile.names
      : tile.depth === 2
        ? node
          ? tile.names.slice(0, -1)
          : tile.names
        : null;
    if (names && names.length > path.length) onOpen(names);
  }
  function menu(event: MouseEvent) {
    const tile = pick(event);
    if (!tile) return;
    event.preventDefault();
    onMenu(fullPath(tile), event.clientX, event.clientY);
  }
  const tipLeft = $derived(
    hover ? tooltipLeft(hover.x, tipWidth, width, 12) : 0,
  );
  const tipTop = $derived(
    hover
      ? hover.y + 16 + tipHeight > height
        ? Math.max(0, hover.y - tipHeight - 10)
        : hover.y + 16
      : 0,
  );
  const share = (tile: Tile) =>
    tile.parent.s > 0
      ? ((tile.block.size / tile.parent.s) * 100).toFixed(1)
      : "0";
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="map"
  bind:clientWidth={width}
  bind:clientHeight={height}
  role="tree"
  tabindex="-1"
  aria-label={$t("space.mapAria")}
  onmousemove={move}
  onmouseleave={() => (hover = null)}
  onclick={click}
  oncontextmenu={menu}
>
  {#each tiles as tile, i (tile.key)}
    <div
      class="tile"
      class:inner={tile.depth === 2}
      class:folder={tile.block.node?.d}
      class:hot={hover?.tile === tile}
      data-tile={i}
      role="treeitem"
      aria-selected="false"
      aria-label={`${label(tile)} · ${formatBytes(tile.block.size)}`}
      style={`left:${tile.x}px;top:${tile.y}px;width:${tile.w}px;height:${tile.h}px;background:${color(tile)}`}
    >
      {#if tile.w >= 44 && tile.h >= 16}
        <span class="name">{label(tile)}</span>
        {#if tile.h >= 32 && !(tile.depth === 1 && tile.block.node?.d && tile.h >= 44)}<span
            class="size">{formatBytes(tile.block.size)}</span
          >{:else if tile.depth === 1 && tile.block.node?.d}<span
            class="size head">{formatBytes(tile.block.size)}</span
          >{/if}
      {/if}
    </div>
  {/each}
  {#if hover}
    <div
      class="tip"
      role="tooltip"
      bind:clientWidth={tipWidth}
      bind:clientHeight={tipHeight}
      style={`left:${tipLeft}px;top:${tipTop}px`}
    >
      <div class="path">{fullPath(hover.tile)}</div>
      {#if !hover.tile.block.node}<div>{label(hover.tile)}</div>{/if}
      <div>
        {formatBytes(hover.tile.block.size)} · {$t("space.shareOf", {
          v: share(hover.tile),
        })}
        {#if !hover.tile.block.node}· {$t("space.items", {
            n: hover.tile.block.count,
          })}{:else if hover.tile.block.node.d}· {$t("space.files", {
            n: hover.tile.block.node.f.toLocaleString(),
          })}{/if}
        {#if hover.tile.block.node?.x}· {$t("space.denied")}{/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .map {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: 8px;
    background: var(--card);
    user-select: none;
  }
  .tile {
    position: absolute;
    box-sizing: border-box;
    overflow: hidden;
    padding: 2px 4px;
    border: 1px solid color-mix(in oklab, var(--foreground) 18%, transparent);
    color: var(--foreground);
    font-size: 11px;
    line-height: 14px;
    cursor: default;
  }
  .tile.folder {
    cursor: pointer;
  }
  .tile.inner {
    border-color: color-mix(in oklab, var(--foreground) 12%, transparent);
    font-size: 10px;
    line-height: 12px;
  }
  .tile.hot {
    outline: 2px solid var(--ring, var(--primary));
    outline-offset: -2px;
    z-index: 1;
  }
  .name,
  .size {
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tile:not(.inner).folder .name {
    font-weight: 600;
    padding-right: 64px;
  }
  .size {
    color: color-mix(in oklab, var(--foreground) 70%, transparent);
    font-variant-numeric: tabular-nums;
  }
  .size.head {
    position: absolute;
    top: 2px;
    right: 4px;
  }
  .tip {
    position: absolute;
    z-index: 5;
    max-width: min(480px, 90%);
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--popover);
    color: var(--popover-foreground);
    font-size: 11.5px;
    line-height: 16px;
    pointer-events: none;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.18);
    font-variant-numeric: tabular-nums;
  }
  .path {
    font-weight: 600;
    word-break: break-all;
  }
</style>
