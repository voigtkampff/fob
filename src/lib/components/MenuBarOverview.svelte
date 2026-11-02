<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Pause, Play, Maximize2, Power } from "@lucide/svelte";
  import Dashboard from "./dashboard/Dashboard.svelte";
  import type { DetailKind } from "./dashboard/detail";
  import { themeStore } from "$lib/stores/theme";
  import { t } from "$lib/i18n";
  import { miniView, needsFit } from "$lib/utils/mini-fit.js";
  import type { OverviewSnapshot } from "$lib/types";
  let data: OverviewSnapshot | null = null;
  let error = "";
  let visible = false;
  let destroyed = false;
  let clock = Date.now();
  let gpuId = "";
  let inFlight: Promise<void> | null = null;
  let poll: ReturnType<typeof setTimeout>;
  let ticker: ReturnType<typeof setInterval>;
  const listeners: UnlistenFn[] = [];
  // winperf: the window is sized to the panel once per view (cards <-> a
  // detail), never on data refreshes; see utils/mini-fit.js.
  let panel: HTMLDivElement;
  let detail: DetailKind | null = null;
  let fitted: string | null = null;
  $: view = miniView(!!data, detail);
  $: if (needsFit(fitted, view)) void fit(view);
  const frame = () => new Promise((r) => requestAnimationFrame(r));
  function refitAfterClick() {
    void tick().then(() => {
      if (fitted === view) fitted = null; // the reactive fit runs again
    });
  }
  async function fit(target: string) {
    fitted = target;
    let asked = 0;
    // Pass 2 corrects for whatever the native window took off the client
    // area (frame, DPI rounding), so the cards view never needs a scrollbar.
    for (let pass = 0; pass < 2; pass++) {
      await tick();
      await frame();
      if (!panel || fitted !== target) return;
      panel.scrollTop = 0;
      const content = Math.ceil(panel.scrollHeight);
      const short = content - window.innerHeight;
      if (pass === 1 && short <= 0) return;
      asked = pass === 0 ? content : asked + short;
      await invoke("fit_companion", { height: asked }).catch(() => {});
      await new Promise((r) => setTimeout(r, 120));
    }
  }
  $: age = data
    ? Math.max(0, Math.floor((clock - data.captured_at) / 1000))
    : 0;
  $: stale =
    !!data && clock - data.captured_at > Math.max(10000, data.interval * 3);
  async function fetchOverview() {
    try {
      const next = await invoke<OverviewSnapshot>("get_overview");
      if (!destroyed) {
        if (!next.gpus.some((g) => g.id === gpuId))
          gpuId =
            next.gpus.find((g) => g.utilization !== null)?.id ??
            next.gpus[0]?.id ??
            "";
        data = next;
        error = next.error ?? "";
        clock = Date.now();
      }
    } catch (e) {
      if (!destroyed) error = String(e);
    }
  }
  function read() {
    if (inFlight) return inFlight;
    inFlight = fetchOverview().finally(() => {
      inFlight = null;
    });
    return inFlight;
  }
  function schedule() {
    clearTimeout(poll);
    if (!visible || destroyed) return;
    poll = setTimeout(
      async () => {
        await read();
        schedule();
      },
      Math.min(data?.interval ?? 3000, 3000),
    );
  }
  async function visibility(value: boolean) {
    visible = value;
    if (!visible) {
      detail = null; // reopen on the cards
      fitted = null; // and measure them once more while hidden
    }
    clearTimeout(poll);
    clearInterval(ticker);
    if (visible && !destroyed) {
      ticker = setInterval(() => {
        clock = Date.now();
      }, 1000);
      await read();
      schedule();
    }
  }
  async function pause() {
    try {
      await invoke("set_monitor_paused", { paused: !data?.paused });
      await read();
    } catch (e) {
      error = String(e);
    }
  }
  async function open(process?: { pid: number; start_time: number }) {
    try {
      await invoke("open_main_window", { process: process ?? null });
    } catch (e) {
      error = String(e);
    }
  }
  onMount(() => {
    document.documentElement.dataset.companion = "true";
    themeStore.init();
    void (async () => {
      for (const [event, callback] of [
        [
          "companion-visibility",
          ({ payload }: { payload: boolean }) => {
            void visibility(payload);
          },
        ],
        [
          "monitor-control",
          () => {
            if (visible) void read();
          },
        ],
      ] as const) {
        const stop = await listen(event, callback);
        if (destroyed) stop();
        else listeners.push(stop);
      }
      // Listen before reading visibility, so a popup opened during startup is not lost.
      const isVisible = await getCurrentWindow().isVisible();
      if (!destroyed) void visibility(isVisible);
    })();
  });
  onDestroy(() => {
    destroyed = true;
    clearTimeout(poll);
    clearInterval(ticker);
    listeners.forEach((stop) => stop());
  });
</script>

<svelte:window
  on:keydown={(event) => {
    if (event.key !== "Escape") return;
    // winperf: Esc closes an open detail view first, then hides the window.
    if (detail) detail = null;
    else void invoke("hide_companion");
  }}
/>

<div class="mini-shell">
  <!-- A click can open or close part of a view (the sensor setup steps):
       measure once more after it. Data refreshes never resize. -->
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="mini" bind:this={panel} on:click={refitAfterClick}>
    <div class="mini-bar">
      <img src="/fob.svg" alt="" class="size-4" />
      <span class="mini-title">{$t("app.name")}</span>
      <span class="mini-status"
        >{data?.paused
          ? $t("mini.paused")
          : stale
            ? $t("mini.lastReading", { s: age })
            : data
              ? $t("mini.live")
              : $t("mini.reading")}</span
      >
      <span class="flex-1"></span>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={data?.paused ? $t("mini.resume") : $t("mini.pause")}
        title={data?.paused ? $t("mini.resume") : $t("mini.pause")}
        onclick={pause}
        disabled={!data}
        >{#if data?.paused}<Play />{:else}<Pause />{/if}</Button
      >
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={$t("mini.openFull")}
        title={$t("mini.openFull")}
        onclick={() => open()}><Maximize2 /></Button
      >
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={$t("mini.quit")}
        title={$t("mini.quit")}
        onclick={() => invoke("quit_app")}><Power /></Button
      >
    </div>
    <div>
      {#if error}<div role="alert" class="mini-error">
          {error}<Button variant="link" size="sm" onclick={read}
            >{$t("mini.retry")}</Button
          >
        </div>{/if}
      {#if data}
        <Dashboard
          compact
          expandable
          bind:detail
          extra={data.extra}
          topCpu={data.top_cpu}
          topGpu={data.top_gpu}
          topMemory={data.top_memory}
          systemStats={data.stats}
          gpus={data.gpus}
          history={data.history}
          processes={data.processes}
        />
      {:else}
        <p class="p-4 text-xs text-muted-foreground">
          {$t("mini.readingMachine")}
        </p>
      {/if}
    </div>
  </div>
</div>

<style>
  /* winperf mini window: the panel is the whole (opaque) window. Corners and
     shadow come from Windows 11 (DWM), so there is no CSS radius, border or
     shadow here; see companion::native_frame. */
  .mini-shell {
    height: 100vh;
  }
  .mini {
    max-height: 100vh;
    overflow-y: auto;
    /* Only long detail lists scroll; keep the bar thin and trackless. */
    scrollbar-width: thin;
    scrollbar-color: color-mix(
        in oklab,
        var(--muted-foreground) 45%,
        transparent
      )
      transparent;
    background: var(--background);
    padding: 8px 10px 10px;
  }
  .mini-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }
  .mini-title {
    font-size: 13px;
    font-weight: 650;
  }
  .mini-status {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .mini-error {
    font-size: 12px;
    color: var(--destructive);
    margin-bottom: 6px;
  }
</style>
