<script lang="ts">
  // Fob: logical processors laid out by count (see utils/cpu-layout.js):
  // labeled bars, heat cells, or the average plus the busiest threads.
  // On a hybrid CPU (`groups`) each core group gets its own labeled block.
  import {
    coreLayout,
    busiestThreads,
    CARD_ROW,
    CARD_GAP,
  } from "$lib/utils/cpu-layout.js";
  import { groupPercent, type CpuGroup } from "$lib/utils/cpu-groups.js";
  import { t } from "$lib/i18n";
  let {
    usage,
    where = "card",
    groups = null,
  }: {
    usage: number[];
    where?: "card" | "detail";
    groups?: CpuGroup[] | null;
  } = $props();
  const layout = $derived(coreLayout(usage.length, where));
  const clamp = (v: number) =>
    Number.isFinite(v) ? Math.max(0, Math.min(100, v)) : 0;
  const tip = (i: number, v: number) =>
    $t("cpu.threadTip", {
      i,
      v: Number.isFinite(v) ? v.toFixed(0) : "—",
    });
  const groupTip = (g: CpuGroup) =>
    g.usage == null
      ? $t("cpu.groupNoReading", { g: $t(g.label) })
      : $t("cpu.groupTip", {
          g: $t(g.label),
          n: g.threads.length,
          c: g.cores,
        });
  const heat = (v: number) =>
    `background:color-mix(in oklab, var(--chart-1) ${Math.round(15 + clamp(v) * 0.85)}%, var(--muted))`;
  const average = $derived(
    usage.length ? usage.reduce((a, b) => a + clamp(b), 0) / usage.length : 0,
  );
</script>

{#snippet grid(threads: number[], l: ReturnType<typeof coreLayout>)}
  {#if l.mode !== "summary"}
    <div
      class={`cores ${l.mode} ${where}`}
      style={`grid-template-columns:repeat(${l.cols},minmax(0,1fr));gap:${where === "card" ? CARD_GAP[l.mode] : 3}px;--row:${where === "card" ? CARD_ROW[l.mode] : l.mode === "cells" ? 54 : 14}px`}
    >
      {#each threads as i (i)}
        {#if l.mode === "cells"}
          <div class="cell" title={tip(i, usage[i])}>
            <div class="bar">
              <span style={`height:${Math.max(4, clamp(usage[i]))}%`}></span>
            </div>
            <small>{i}</small>
          </div>
        {:else}
          <div
            class="heat"
            title={tip(i, usage[i])}
            style={heat(usage[i])}
          ></div>
        {/if}
      {/each}
    </div>
  {/if}
{/snippet}

{#if !usage.length}
  <p class="text-xs text-muted-foreground">{$t("cpu.noCores")}</p>
{:else if groups && where === "card"}
  <div class="group-strips" role="group" aria-label={$t("dash.perCore")}>
    {#each groups as g (g.kind)}
      <div class="strip-group" title={groupTip(g)}>
        <div class="line">
          <span class="truncate">{$t(g.label)}</span><span
            class="num"
            class:none={g.usage == null}
            >{groupPercent(g.usage)}{g.usage == null ? "" : "%"}</span
          >
        </div>
        <div
          class="strip"
          style={`grid-template-columns:repeat(${g.threads.length},minmax(0,1fr))`}
        >
          {#each g.threads as i (i)}
            <div
              class="heat"
              title={tip(i, usage[i])}
              style={heat(usage[i])}
            ></div>
          {/each}
        </div>
      </div>
    {/each}
  </div>
{:else if groups}
  <div class="group-blocks" role="group" aria-label={$t("dash.perCore")}>
    {#each groups as g (g.kind)}
      <section class="group-block">
        <header class="group-head" title={groupTip(g)}>
          <span class="group-name">{$t(g.label)}</span>
          <span class="num" class:none={g.usage == null}
            >{groupPercent(g.usage)}{g.usage == null ? "" : "%"}</span
          >
          <span class="group-size"
            >{$t("detail.coresThreads", {
              c: g.cores,
              n: g.threads.length,
            })}</span
          >
        </header>
        {@render grid(g.threads, coreLayout(g.threads.length, "detail"))}
      </section>
    {/each}
  </div>
{:else if layout.mode === "summary"}
  <div class="summary">
    <div class="line">
      <span>{$t("cpu.allAvg", { n: usage.length })}</span><span class="num"
        >{average.toFixed(0)}%</span
      >
    </div>
    {#each busiestThreads(usage, layout.top) as c (c.index)}
      <div class="line">
        <span>{$t("cpu.thread", { i: c.index })}</span><span class="num"
          >{c.value.toFixed(0)}%</span
        >
      </div>
    {/each}
  </div>
{:else}
  <div role="group" aria-label={$t("dash.perCore")}>
    {@render grid(
      usage.map((_, i) => i),
      layout,
    )}
  </div>
{/if}

<style>
  .cores {
    display: grid;
    grid-auto-rows: var(--row);
  }
  .cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
    height: var(--row);
  }
  .bar {
    flex: 1;
    width: 100%;
    display: flex;
    align-items: flex-end;
    border-radius: 2px;
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
    overflow: hidden;
  }
  .bar span {
    display: block;
    width: 100%;
    background: var(--chart-1);
  }
  small {
    font-size: 8px;
    line-height: 9px;
    color: var(--muted-foreground);
    font-variant-numeric: tabular-nums;
  }
  .detail small {
    font-size: 9px;
    line-height: 12px;
  }
  .heat {
    border-radius: 2px;
    height: 100%;
  }
  /* Four 14 px lines (56 px) fit the card's 60 px middle row. */
  .summary {
    display: flex;
    flex-direction: column;
    font-size: 11px;
    line-height: 14px;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 6px;
    color: var(--muted-foreground);
  }
  .num {
    color: var(--foreground);
    font-variant-numeric: tabular-nums;
  }
  .num.none {
    color: var(--muted-foreground);
  }
  /* Card: two groups of a 14 px line and a 10 px strip (52 px) fit the
     card's 56 px core area. */
  .group-strips {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11px;
    line-height: 14px;
  }
  .strip {
    display: grid;
    gap: 1px;
    height: 10px;
  }
  .group-blocks {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .group-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 4px;
    font-size: 12px;
    line-height: 16px;
  }
  .group-name {
    font-weight: 600;
  }
  .group-size {
    color: var(--muted-foreground);
    font-size: 11px;
  }
</style>
