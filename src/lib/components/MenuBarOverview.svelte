<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Pause, Play, Maximize2, Power } from "@lucide/svelte";
  import Dashboard from "./dashboard/Dashboard.svelte";
  import type { DetailKind } from "./dashboard/detail";
  import { themeStore } from "$lib/stores/theme";
  import { t } from "$lib/i18n";
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
  // winperf: the window height follows the panel's content (clamped natively).
  let panel: HTMLDivElement;
  // Stable wrapper: Dashboard swaps its root elements when a detail opens.
  let body: HTMLDivElement;
  let detail: DetailKind | null = null;
  let resize: ResizeObserver | undefined;
  let lastHeight = 0;
  function fit() {
    if (!panel) return;
    const h = Math.ceil(panel.scrollHeight);
    if (Math.abs(h - lastHeight) < 2) return;
    lastHeight = h;
    void invoke("fit_companion", { height: h }).catch(() => {});
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
    if (!visible) detail = null; // reopen on the cards
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
    resize = new ResizeObserver(fit);
    if (body) resize.observe(body);
    if (panel) resize.observe(panel);
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
    resize?.disconnect();
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
  <div class="mini" bind:this={panel}>
    <div class="mini-bar">
      <img src="/32x32.png" alt="" class="size-4" />
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
    <div bind:this={body}>
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
