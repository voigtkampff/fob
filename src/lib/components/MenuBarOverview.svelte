<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Pause, Play, Maximize2, Power } from "@lucide/svelte";
  import Dashboard from "./dashboard/Dashboard.svelte";
  import { themeStore } from "$lib/stores/theme";
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
  let resize: ResizeObserver | undefined;
  let lastHeight = 0;
  function fit() {
    if (!panel) return;
    const h = Math.ceil(panel.scrollHeight + 14);
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
    for (const child of panel?.children ?? []) resize.observe(child);
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
    if (event.key === "Escape") void invoke("hide_companion");
  }}
/>

<div class="mini-shell">
  <div class="mini" bind:this={panel}>
    <div class="mini-bar">
      <img src="/32x32.png" alt="" class="size-4" />
      <span class="mini-title">winperf</span>
      <span class="mini-status"
        >{data?.paused
          ? "Paused"
          : stale
            ? `Last reading ${age}s ago`
            : data
              ? "Live"
              : "Reading…"}</span
      >
      <span class="flex-1"></span>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={data?.paused ? "Resume updates" : "Pause updates"}
        title={data?.paused ? "Resume updates" : "Pause updates"}
        onclick={pause}
        disabled={!data}
        >{#if data?.paused}<Play />{:else}<Pause />{/if}</Button
      >
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Open full window"
        title="Open full window"
        onclick={() => open()}><Maximize2 /></Button
      >
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Quit winperf"
        title="Quit winperf"
        onclick={() => invoke("quit_app")}><Power /></Button
      >
    </div>
    {#if error}<div role="alert" class="mini-error">
        {error}<Button variant="link" size="sm" onclick={read}>Retry</Button>
      </div>{/if}
    {#if data}
      <Dashboard
        compact
        systemStats={data.stats}
        gpus={data.gpus}
        history={data.history}
        processes={data.processes}
      />
    {:else}
      <p class="p-4 text-xs text-muted-foreground">Reading this machine…</p>
    {/if}
  </div>
</div>

<style>
  /* winperf mini window: transparent webview, rounded dark panel inside. */
  :global(html[data-companion="true"]),
  :global(html[data-companion="true"] body) {
    background: transparent;
  }
  .mini-shell {
    height: 100vh;
    padding: 6px;
  }
  .mini {
    max-height: calc(100vh - 12px);
    overflow-y: auto;
    background: var(--background);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 8px 10px 10px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
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
