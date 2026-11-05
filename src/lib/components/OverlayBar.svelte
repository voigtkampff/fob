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
    overlayTheme,
    overlayColors,
    inkShift,
    type Reading,
  } from "$lib/utils/overlay.js";
  import { sparklinePath } from "$lib/utils/dashboard.js";
  import { themes } from "$lib/definitions/themes";
  import { themeTokens } from "$lib/utils/theme-tokens.js";
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
  // Its own appearance or the main window's theme (theme-change keeps
  // $themeStore current); only the bar's few colours come from it.
  const theme = $derived(
    overlayTheme($settingsStore.behavior.overlayTheme, $themeStore, themes),
  );
  const colors = $derived(
    Object.entries(
      overlayColors(themeTokens(theme.colors, { name: theme.name })),
    )
      .map(([k, v]) => `${k}:${v}`)
      .join(";"),
  );
  const history = $derived((reading as Reading | null)?.history ?? []);
  // CJK labels get their own size (ideographs fill the em box, so at the
  // Latin size they look as big as the values).
  const CJK = /[\u3000-\u9fff]/;
  // ↓ / ↑ drawn as paths: the glyphs come from whichever font has them
  // (Segoe UI Symbol on Windows), each with its own size and baseline.
  const ARROW: Record<string, string> = {
    "↓": "M4 0.5V8.5M1 5.5L4 8.5L7 5.5",
    "↑": "M4 9.5V1.5M1 4.5L4 1.5L7 4.5",
  };
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
  let bar: HTMLDivElement | undefined = $state();
  // Each script's ink centred on the bar's centre line, from the metrics of
  // the font the WebView really picked (YaHei UI on Windows has far taller
  // ascent/descent than Segoe UI, so a centred line box isn't centred ink).
  function centreInk() {
    const ctx = document.createElement("canvas").getContext("2d");
    if (!ctx || !bar) return;
    const samples: [string, string, string][] = [
      ["--ob-shift-value", ".part", "0123456789"],
      ["--ob-shift-latin", ".label:not(.cjk)", "CPUGRAMVDISKNETPWR"],
      ["--ob-shift-cjk", ".label.cjk", "功耗内存显存网络磁盘"],
    ];
    for (const [name, selector, text] of samples) {
      const el = bar.querySelector<HTMLElement>(selector);
      if (!el) continue;
      const style = getComputedStyle(el);
      ctx.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      const shift = inkShift(parseFloat(style.fontSize), ctx.measureText(text));
      bar.style.setProperty(name, `${shift.toFixed(2)}px`);
    }
  }
  $effect(() => {
    void shown;
    void tick().then(() => document.fonts.ready.then(centreInk));
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
  bind:this={bar}
  class="bar"
  class:interactive
  style={colors}
  data-theme={theme.name}
  style:opacity={config.opacity / 100}
  onmousedown={(e) => {
    if (interactive && e.button === 0) void getCurrentWindow().startDragging();
  }}
>
  {#key paint}<div class="row" bind:this={row}>
      {#each shown as cell (cell.id)}
        {@const chart = config.charts.includes(cell.id)
          ? chartFor(cell.id, history)
          : null}
        <span class="cell"
          ><span
            class="label"
            class:cjk={CJK.test($t(`overlay.short.${cell.id}`))}
            >{$t(`overlay.short.${cell.id}`)}</span
          ><span class="vc {cell.id}" class:charted={chart}
            ><span class="value {cell.id}"
              >{#each cell.parts as part, i (i)}<span class="part"
                  >{#if ARROW[part[0]]}<svg
                      class="arrow"
                      viewBox="0 0 8 10"
                      aria-label={part[0]}><path d={ARROW[part[0]]} /></svg
                    >{part.slice(1)}{:else}{part}{/if}</span
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
          ></span
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
    --ob-label-size-cjk: 11px;
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
    /* Ink centring per script, measured at run time from the fonts the
       bar actually gets (inkShift in utils/overlay.js); 0 until then. */
    --ob-shift-value: 0px;
    --ob-shift-latin: 0px;
    --ob-shift-cjk: 0px;

    height: var(--ob-height);
    max-height: 100vh;
    display: flex;
    align-items: center;
    box-sizing: border-box;
    background: var(--ob-bg);
    border: 1px solid transparent;
    border-radius: 4px;
    user-select: none;
    cursor: default;
    /* Windows first, named outright: WebView2 otherwise falls back per
       glyph. Selawik (Segoe UI metrics) and Noto Sans CJK SC are the
       closest Linux stand-ins, for the screenshots. */
    --ob-font-latin: "Segoe UI", Selawik;
    --ob-font-cjk: "Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans CJK SC";
    font-family: var(--ob-font-latin), var(--ob-font-cjk), sans-serif;
    line-height: 1;
  }
  /* Ctrl held (not 锁定位置): a slightly brighter bar with a faint solid edge. */
  .bar.interactive {
    cursor: move;
    background: color-mix(in oklab, var(--ob-bg), var(--ob-fg) 8%);
    border-color: color-mix(in oklab, var(--ob-border), var(--ob-fg) 15%);
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
    color: var(--ob-accent);
    font-weight: var(--ob-label-weight);
    font-size: var(--ob-label-size);
  }
  .label.cjk {
    font-family: var(--ob-font-cjk), sans-serif;
    font-size: var(--ob-label-size-cjk);
  }
  .arrow {
    width: 7px;
    height: 9px;
    margin-right: 1px;
    flex: none;
  }
  .arrow path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .part {
    transform: translateY(var(--ob-shift-value));
  }
  .label {
    transform: translateY(var(--ob-shift-latin));
  }
  .label.cjk {
    transform: translateY(var(--ob-shift-cjk));
  }
  .value {
    display: inline-flex;
    align-items: center;
    justify-content: flex-start;
    gap: var(--ob-gap-pair);
    height: var(--ob-box);
    /* Left-aligned: fixed widths keep the bar steady, and their slack goes
       after the number, so label → value stays --ob-gap-label. */
    color: var(--ob-fg);
    font-size: var(--ob-value-size);
    font-weight: var(--ob-value-weight);
    font-family: var(--ob-font-latin), sans-serif;
    font-variant-numeric: tabular-nums;
    font-feature-settings: "tnum" 1;
  }
  .part {
    justify-content: flex-start;
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
    stroke: var(--ob-chart);
    stroke-width: 1.25;
    vector-effect: non-scaling-stroke;
  }
  /* Fixed widths, so the bar doesn't twitch as numbers change. The width
     is on the value plus its chart: values in a pair stay --ob-gap-pair
     apart, the chart follows the last value at --ob-gap-chart, and the
     slack falls after the chart. */
  .vc {
    display: inline-flex;
    align-items: center;
    gap: var(--ob-gap-label);
    height: var(--ob-box);
    font-size: var(--ob-value-size);
    font-weight: var(--ob-value-weight);
    font-family: var(--ob-font-latin), sans-serif;
    min-width: var(--ob-value-w);
  }
  .vc.charted {
    min-width: calc(
      var(--ob-value-w) + var(--ob-gap-chart) + var(--ob-chart-w)
    );
  }
  .vc.cpu,
  .vc.gpu {
    --ob-value-w: 4ch;
  }
  .vc.cpuTemp,
  .vc.gpuTemp,
  .vc.power {
    --ob-value-w: 5ch;
  }
  .vc.ram,
  .vc.vram {
    --ob-value-w: 5.5ch;
  }
  .vc.net {
    --ob-value-w: 13ch;
  }
  .vc.disk {
    --ob-value-w: 15ch;
  }
</style>
