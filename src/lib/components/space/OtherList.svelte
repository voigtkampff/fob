<script lang="ts">
  // Fob: the items of a folder's "other" block, largest first. Items the
  // page merged come from the scan; items the scan itself folded (their
  // names are not cached) are listed and measured by the backend now, in
  // pages. Only the rows in view are drawn, so long lists stay light.
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { X, Folder, File } from "@lucide/svelte";
  import type { SpaceNode, Block } from "$lib/utils/treemap.js";
  import { formatBytes } from "$lib/utils";
  import { t } from "$lib/i18n";

  type Item = { n: string; s: number; f: number; d: boolean; t: boolean };
  let {
    title,
    fullPath,
    parent,
    block,
    onDrill,
    onMenu,
    onClose,
  }: {
    title: string;
    fullPath: string;
    parent: SpaceNode;
    block: Block;
    onDrill: (name: string) => void;
    onMenu: (name: string, x: number, y: number) => void;
    onClose: () => void;
  } = $props();

  const ROW = 30;
  const PAGE = 200;
  const merged = $derived(block.merged ?? []);
  const local = $derived(!(parent.oc ?? 0) || !isTauri());
  let items = $state<(Item | undefined)[]>([]);
  let total = $state(0);
  let error = $state("");
  let loading = $state(false);
  let scrollTop = $state(0);
  let viewH = $state(400);
  const requested = new Set<number>();

  const known = $derived(
    merged.map((m) => ({ n: m.n, s: m.s, f: m.f, d: !!m.d, t: true })),
  );
  $effect(() => {
    requested.clear();
    error = "";
    if (local) {
      items = [...known].sort((a, b) => b.s - a.s);
      total = items.length;
    } else {
      items = [];
      total = block.count;
      void page(0);
    }
  });
  async function page(index: number) {
    if (local || requested.has(index)) return;
    requested.add(index);
    loading = true;
    try {
      const shownNames = new Set(merged.map((m) => m.n));
      const res = await invoke<{ total: number; items: Item[] }>(
        "space_folded",
        {
          path: fullPath,
          shown: (parent.c ?? [])
            .filter((c) => !shownNames.has(c.n))
            .map((c) => c.n),
          known,
          offset: index * PAGE,
          limit: PAGE,
        },
      );
      total = res.total;
      const next = items.slice();
      res.items.forEach((it, i) => (next[index * PAGE + i] = it));
      items = next;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 5));
  const last = $derived(
    Math.min(total, Math.ceil((scrollTop + viewH) / ROW) + 5),
  );
  $effect(() => {
    for (let p = Math.floor(first / PAGE); p * PAGE < last; p++) void page(p);
  });
  const share = (s: number) =>
    parent.s > 0 ? ((s / parent.s) * 100).toFixed(2) : "0";
  const rows = $derived(
    Array.from({ length: Math.max(0, last - first) }, (_, i) => first + i),
  );
</script>

<div class="other" role="dialog" aria-label={title}>
  <header>
    <div class="min-w-0">
      <h3>{title}</h3>
      <p class="note">
        {$t("space.otherNote", {
          n: total.toLocaleString(),
          size: formatBytes(block.size),
        })}
      </p>
    </div>
    <button class="close" onclick={onClose} aria-label={$t("space.otherClose")}
      ><X class="size-4" /></button
    >
  </header>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div
    class="scroll"
    bind:clientHeight={viewH}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  >
    <div class="spacer" style:height={`${total * ROW}px`}>
      {#each rows as i (i)}
        {@const it = items[i]}
        <div class="row" style:top={`${i * ROW}px`}>
          {#if it}
            <button
              class="item"
              class:drill={it.d && it.t}
              disabled={!(it.d && it.t)}
              title={it.d && !it.t ? $t("space.notInMap") : it.n}
              onclick={() => onDrill(it.n)}
              oncontextmenu={(e) => {
                e.preventDefault();
                onMenu(it.n, e.clientX, e.clientY);
              }}
            >
              {#if it.d}<Folder class="size-3.5 shrink-0" />{:else}<File
                  class="size-3.5 shrink-0"
                />{/if}
              <span class="name">{it.n}</span>
              <span class="num">{formatBytes(it.s)}</span>
              <span class="num pct">{share(it.s)}%</span>
            </button>
          {:else}
            <span class="pending"
              >{loading ? $t("space.otherLoading") : ""}</span
            >
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .other {
    position: absolute;
    top: 8px;
    right: 8px;
    bottom: 8px;
    z-index: 6;
    width: min(440px, 70%);
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--popover);
    color: var(--popover-foreground);
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.25);
  }
  header {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    justify-content: space-between;
    padding: 10px 12px 8px;
    border-bottom: 1px solid var(--border);
  }
  h3 {
    font-size: 13px;
    font-weight: 650;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .note,
  .pending {
    font-size: 11.5px;
    color: var(--muted-foreground);
  }
  .error {
    padding: 4px 12px;
    font-size: 12px;
    color: var(--destructive);
  }
  .close {
    padding: 4px;
    border-radius: 6px;
  }
  .close:hover {
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .spacer {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 30px;
    display: flex;
    align-items: center;
    padding: 0 8px;
  }
  .item {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) 5.5rem 4rem;
    gap: 8px;
    align-items: center;
    width: 100%;
    height: 26px;
    padding: 0 6px;
    border-radius: 6px;
    font-size: 12.5px;
    text-align: left;
    color: inherit;
  }
  .item:disabled {
    cursor: default;
    opacity: 1;
  }
  .item.drill {
    cursor: pointer;
  }
  .item.drill:hover {
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .pct {
    color: var(--muted-foreground);
  }
</style>
