<script lang="ts">
  // Fob status bar (overlay.rs): one row of readings in a thin always-on-top
  // window. Colours come from the current theme (background, accent label,
  // foreground value), no shadows; the opacity setting fades the whole bar,
  // text and charts included (0.7.1).
  import { onDestroy, onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { settingsStore } from "$lib/stores/settings";
  import { themeStore } from "$lib/stores/theme";
  import {
    overlaySettings,
    cells,
    chartFor,
    type Reading,
  } from "$lib/utils/overlay.js";
  import { sparklinePath } from "$lib/utils/dashboard.js";
  import { t } from "$lib/i18n";

  let reading: Reading | null = $state(null);
  let interactive = $state(false);
  let row: HTMLDivElement | undefined = $state();
  let lastWidth = 0;
  // Bumped after each resize: WebKitGTK (Linux) repaints only the changed
  // text of a resized transparent window; a fresh row is painted whole.
  let paint = $state(0);
  const config = $derived(overlaySettings($settingsStore.behavior));
  const shown = $derived(cells(reading, config.metrics));
  const history = $derived((reading as Reading | null)?.history ?? []);
  // Chart size: matches --ob-chart-w / --ob-chart-h below.
  const CW = 32;
  const CH = 9;
  const stops: UnlistenFn[] = [];
  let timer: ReturnType<typeof setInterval> | undefined;
  let destroyed = false;

  async function read() {
    try {
      reading = await invoke<Reading>("overlay_reading");
    } catch {
      reading = null;
    }
  }
  // Size the window to the row once it changes by a few pixels.
  $effect(() => {
    void shown;
    void tick().then(() => {
      const width = Math.ceil(row?.getBoundingClientRect().width ?? 0);
      if (width && Math.abs(width - lastWidth) >= 4) {
        lastWidth = width;
        void invoke("overlay_fit", { width })
          .then(() => setTimeout(() => paint++, 150))
          .catch(() => {});
      }
    });
  });
  onMount(() => {
    document.documentElement.dataset.overlay = "true";
    void settingsStore.init();
    themeStore.init();
    void read();
    timer = setInterval(read, 1000);
    void listen<boolean>("overlay-interactive", (e) => {
      interactive = e.payload;
    }).then(async (stop) => {
      if (destroyed) return stop();
      stops.push(stop);
      // A change sent before this listener existed would be lost.
      interactive = await invoke<boolean>("overlay_interactive").catch(
        () => false,
      );
    });
  });
  onDestroy(() => {
    destroyed = true;
    clearInterval(timer);
    stops.forEach((stop) => stop());
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="bar"
  class:interactive
  style:opacity={config.opacity / 100}
  onmousedown={(e) => {
    if (interactive && e.button === 0) void getCurrentWindow().startDragging();
  }}
>
  {#key paint}<div class="row" bind:this={row}>
      {#each shown as cell (cell.id)}
        {@const chart = config.charts ? chartFor(cell.id, history) : null}
        <span class="cell"
          ><span class="label">{$t(`overlay.short.${cell.id}`)}</span><span
            class="value {cell.id}"
            >{#each cell.parts as part, i (i)}<span class="part">{part}</span
              >{/each}</span
          >{#if chart}<svg
              class="chart"
              width={CW}
              height={CH}
              viewBox={`0 0 ${CW} ${CH}`}
              preserveAspectRatio="none"
              aria-hidden="true"
              ><path
                d={sparklinePath(history, chart.pick, {
                  width: CW,
                  height: CH,
                  max: chart.max,
                })}
              /></svg
            >{/if}</span
        >
      {:else}
        <span class="label">{$t("overlay.empty")}</span>
      {/each}
    </div>{/key}
</div>

<style>
  :global(html[data-overlay]),
  :global(html[data-overlay] body) {
    background: transparent !important;
    overflow: hidden;
    margin: 0;
  }
  /* Status bar layout standard (0.7.2), the one place its sizes live.
     docs-fob/status-bar-layout.md explains the rules.
     - Bar 26 px; every label, value and chart is a box of --ob-box height
       centred on the bar's one centre line (13 px).
     - Labels: regular weight, --ob-label-size, CJK and Latin alike;
       values: bold tabular figures, --ob-value-size, units in the same
       text so they share its baseline.
     - Charts: --ob-chart-h (under the value's cap height), --ob-chart-w
       wide, inside the same box, never past the text.
     - Gaps: label→value, between the two values of a pair, between
       metrics; nothing else adds space. */
  .bar {
    --ob-height: 26px;
    --ob-box: 14px;
    --ob-label-size: 11px;
    --ob-value-size: 13px;
    --ob-label-weight: 400;
    --ob-value-weight: 700;
    --ob-chart-w: 32px;
    --ob-chart-h: 9px;
    --ob-gap-label: 5px;
    --ob-gap-pair: 8px;
    --ob-gap-chart: 6px;
    --ob-gap-metric: 14px;
    --ob-pad-x: 10px;
    /* Optical centring, measured with the guide screenshots: bold 13 px
       figures (cap height only, no descenders) ink 0.5 px low in a centred
       line box; 11 px labels, CJK or Latin, ink within ±0.25 px of centre. */
    --ob-value-shift: -0.5px;

    height: var(--ob-height);
    max-height: 100vh;
    display: flex;
    align-items: center;
    box-sizing: border-box;
    background: var(--background);
    border: 1px solid transparent;
    border-radius: 4px;
    user-select: none;
    cursor: default;
    font-family: "Segoe UI", "Microsoft YaHei UI", system-ui, sans-serif;
    line-height: 1;
  }
  /* Ctrl held / unlocked: a slightly brighter bar with a faint solid edge. */
  .bar.interactive {
    cursor: move;
    background: color-mix(in oklab, var(--background), var(--foreground) 8%);
    border-color: color-mix(in oklab, var(--border), var(--foreground) 15%);
  }
  .row {
    display: inline-flex;
    align-items: center;
    gap: var(--ob-gap-metric);
    padding: 0 var(--ob-pad-x);
    white-space: nowrap;
  }
  .cell {
    display: inline-flex;
    align-items: center;
    gap: var(--ob-gap-label);
    height: var(--ob-box);
  }
  .label,
  .part {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    height: var(--ob-box);
    line-height: 1;
  }
  .label {
    color: var(--primary);
    font-weight: var(--ob-label-weight);
    font-size: var(--ob-label-size);
  }
  .part {
    transform: translateY(var(--ob-value-shift));
  }
  .value {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--ob-gap-pair);
    height: var(--ob-box);
    color: var(--foreground);
    font-size: var(--ob-value-size);
    font-weight: var(--ob-value-weight);
    font-variant-numeric: tabular-nums;
  }
  .part {
    justify-content: flex-end;
  }
  .chart {
    display: block;
    flex: none;
    width: var(--ob-chart-w);
    height: var(--ob-chart-h);
    margin-left: calc(var(--ob-gap-chart) - var(--ob-gap-label));
    overflow: hidden;
  }
  .chart path {
    fill: none;
    stroke: var(--primary);
    stroke-width: 1.25;
    vector-effect: non-scaling-stroke;
  }
  /* Fixed widths, so the bar doesn't twitch as numbers change. */
  .value.cpu,
  .value.gpu {
    min-width: 4ch;
  }
  .value.cpuTemp,
  .value.gpuTemp,
  .value.power {
    min-width: 5ch;
  }
  .value.ram,
  .value.vram {
    min-width: 5.5ch;
  }
  .value.net .part {
    min-width: 6ch;
  }
  .value.disk .part {
    min-width: 7ch;
  }
</style>
