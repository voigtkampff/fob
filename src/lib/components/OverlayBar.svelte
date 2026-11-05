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
  const CW = 36;
  const CH = 14;
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
                  height: CH - 1,
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
  .bar {
    height: 100vh;
    display: flex;
    align-items: center;
    box-sizing: border-box;
    background: var(--background);
    border: 1px solid transparent;
    border-radius: 4px;
    user-select: none;
    cursor: default;
    font-family: "Segoe UI", "Microsoft YaHei UI", system-ui, sans-serif;
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
    gap: 14px;
    padding: 0 10px;
    white-space: nowrap;
    font-size: 13px;
    line-height: 1;
  }
  .cell {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .label {
    color: var(--primary);
    font-weight: 600;
    font-size: 11px;
  }
  .value {
    display: inline-flex;
    justify-content: flex-end;
    gap: 9px;
    color: var(--foreground);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .chart {
    display: block;
    margin-left: 2px;
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
    text-align: right;
  }
  .value.disk .part {
    min-width: 7ch;
    text-align: right;
  }
</style>
