<script lang="ts">
  // Fob: the hover cursor every history chart shares. Wrap a chart in it:
  // moving the mouse over the chart snaps a vertical line to the nearest
  // sample and shows that sample's value(s) and time in a small tooltip that
  // flips sides near the edges. Everything is absolutely positioned on top of
  // the chart, so hovering never changes the layout.
  import type { Snippet } from "svelte";
  import { cursorAt, tooltipLeft, clockTime } from "$lib/utils/chart-cursor.js";
  import { SPARK_WINDOW_MS } from "$lib/utils/dashboard.js";
  import { t } from "$lib/i18n";

  type Sample = { timestamp: number };
  type Line = {
    pick: (s: any) => number | null | undefined;
    color: string;
    label?: string;
  };
  let {
    history = [],
    lines,
    max,
    format,
    end = undefined,
    windowMs = SPARK_WINDOW_MS,
    inset = { left: 0, right: 0, top: 0, bottom: 0 },
    class: className = "",
    children,
  }: {
    history: Sample[];
    lines: Line[];
    max: number;
    format: (v: number) => string;
    end?: number;
    windowMs?: number;
    inset?: { left: number; right: number; top: number; bottom: number };
    class?: string;
    children?: Snippet;
  } = $props();

  let width = $state(0);
  let height = $state(0);
  let tipWidth = $state(0);
  let ratio = $state<number | null>(null);
  const plotW = $derived(Math.max(1, width - inset.left - inset.right));
  const plotH = $derived(Math.max(1, height - inset.top - inset.bottom));
  const hit = $derived(
    ratio === null ? null : cursorAt(history, ratio, { end, windowMs }),
  );
  const x = $derived(hit ? inset.left + hit.x * plotW : 0);
  const value = (line: Line) => {
    const v = hit ? line.pick(hit.sample) : null;
    return typeof v === "number" && Number.isFinite(v) ? v : null;
  };
  function move(event: PointerEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    ratio = (event.clientX - rect.left - inset.left) / plotW;
  }
</script>

<div
  class={`chart-cursor ${className}`}
  bind:clientWidth={width}
  bind:clientHeight={height}
  onpointermove={move}
  onpointerleave={() => (ratio = null)}
  role="presentation"
>
  {@render children?.()}
  {#if hit}
    <div class="cursor-line" style:left={`${x}px`}></div>
    {#each lines as line}
      {@const v = value(line)}
      {#if v !== null}
        <div
          class="cursor-dot"
          style:left={`${x}px`}
          style:top={`${inset.top + (1 - Math.min(Math.max(v, 0), max) / max) * plotH}px`}
          style:background={line.color}
        ></div>
      {/if}
    {/each}
    <div
      class="cursor-tip"
      bind:clientWidth={tipWidth}
      style:left={`${tooltipLeft(x, tipWidth, width)}px`}
    >
      <div class="cursor-values">
        {#each lines as line, i}
          {@const v = value(line)}
          {#if i}<span class="sep"> </span>{/if}<span
            >{line.label ? `${line.label} ` : ""}{v === null
              ? $t("chart.noReading")
              : format(v)}</span
          >
        {/each}
      </div>
      <div class="cursor-time">{clockTime(hit.sample.timestamp)}</div>
    </div>
  {/if}
</div>

<style>
  .chart-cursor {
    position: relative;
    display: block;
    width: 100%;
    cursor: crosshair;
  }
  .chart-cursor > :global(svg) {
    display: block;
  }
  .cursor-line {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    margin-left: -0.5px;
    background: color-mix(in oklab, var(--foreground) 45%, transparent);
    pointer-events: none;
  }
  .cursor-dot {
    position: absolute;
    width: 6px;
    height: 6px;
    margin: -3px 0 0 -3px;
    border-radius: 50%;
    box-shadow: 0 0 0 1.5px var(--card);
    pointer-events: none;
  }
  .cursor-tip {
    position: absolute;
    top: 0;
    z-index: 5;
    padding: 1px 5px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: color-mix(
      in oklab,
      var(--popover, var(--card)) 94%,
      transparent
    );
    color: var(--foreground);
    font-size: 10px;
    line-height: 12px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.12);
  }
  .cursor-values {
    font-weight: 600;
  }
  .cursor-time {
    color: var(--muted-foreground);
  }
</style>
