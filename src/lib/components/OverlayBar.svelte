<script lang="ts">
  // Fob status bar (overlay.rs): one row of readings in a thin always-on-top
  // window. Fixed light-on-dark colours whatever the theme, no shadows;
  // only the background follows the opacity setting.
  import { onDestroy, onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { settingsStore } from "$lib/stores/settings";
  import { overlaySettings, cells, type Reading } from "$lib/utils/overlay.js";
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
  style:--alpha={config.opacity / 100}
  onmousedown={(e) => {
    if (interactive && e.button === 0) void getCurrentWindow().startDragging();
  }}
>
  {#key paint}<div class="row" bind:this={row}>
      {#each shown as cell (cell.id)}
        <span class="cell"
          ><span class="label">{$t(`overlay.short.${cell.id}`)}</span><span
            class="value {cell.id}">{cell.value}</span
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
  .bar {
    height: 100vh;
    display: flex;
    align-items: center;
    background: rgb(12 14 18 / var(--alpha));
    border-radius: 4px;
    user-select: none;
    cursor: default;
    font-family: "Segoe UI", "Microsoft YaHei UI", system-ui, sans-serif;
  }
  .bar.interactive {
    cursor: move;
    outline: 1px dashed rgb(255 255 255 / 0.75);
    outline-offset: -1px;
  }
  .row {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    white-space: nowrap;
    font-size: 13px;
    line-height: 1;
  }
  .cell {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
  }
  .label {
    color: #9fd3ff;
    font-weight: 600;
    font-size: 11px;
  }
  .value {
    color: #ffffff;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    text-align: right;
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
  .value.net {
    min-width: 13ch;
  }
  .value.disk {
    min-width: 15ch;
  }
</style>
