<script lang="ts">
  // Fob: logical processors laid out by count (see utils/cpu-layout.js):
  // labeled bars, heat cells, or the average plus the busiest threads.
  import {
    coreLayout,
    busiestThreads,
    CARD_ROW,
    CARD_GAP,
  } from "$lib/utils/cpu-layout.js";
  import { t } from "$lib/i18n";
  let {
    usage,
    where = "card",
  }: { usage: number[]; where?: "card" | "detail" } = $props();
  const layout = $derived(coreLayout(usage.length, where));
  const clamp = (v: number) =>
    Number.isFinite(v) ? Math.max(0, Math.min(100, v)) : 0;
  const tip = (i: number, v: number) =>
    $t("cpu.threadTip", {
      i,
      v: Number.isFinite(v) ? v.toFixed(0) : "—",
    });
  const average = $derived(
    usage.length ? usage.reduce((a, b) => a + clamp(b), 0) / usage.length : 0,
  );
</script>

{#if !usage.length}
  <p class="text-xs text-muted-foreground">{$t("cpu.noCores")}</p>
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
  <div
    class={`cores ${layout.mode} ${where}`}
    style={`grid-template-columns:repeat(${layout.cols},minmax(0,1fr));gap:${where === "card" ? CARD_GAP[layout.mode] : 3}px;--row:${where === "card" ? CARD_ROW[layout.mode] : layout.mode === "cells" ? 54 : 14}px`}
    role="group"
    aria-label={$t("dash.perCore")}
  >
    {#each usage as v, i}
      {#if layout.mode === "cells"}
        <div class="cell" title={tip(i, v)}>
          <div class="bar">
            <span style={`height:${Math.max(4, clamp(v))}%`}></span>
          </div>
          <small>{i}</small>
        </div>
      {:else}
        <div
          class="heat"
          title={tip(i, v)}
          style={`background:color-mix(in oklab, var(--chart-1) ${Math.round(15 + clamp(v) * 0.85)}%, var(--muted))`}
        ></div>
      {/if}
    {/each}
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
    color: var(--muted-foreground);
  }
  .num {
    color: var(--foreground);
    font-variant-numeric: tabular-nums;
  }
</style>
