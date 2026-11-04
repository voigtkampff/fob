<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    Pause,
    Play,
    Maximize2,
    Power,
    Pin,
    PinOff,
    Blend,
  } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import OpacitySlider from "./OpacitySlider.svelte";
  import { settingsStore } from "$lib/stores/settings";
  import { miniOpacity, miniTint, inkBoost } from "$lib/utils/mini-opacity.js";
  import Dashboard from "./dashboard/Dashboard.svelte";
  import type { DetailKind } from "./dashboard/detail";
  import { themeStore } from "$lib/stores/theme";
  import { t } from "$lib/i18n";
  import { miniStyle } from "$lib/utils/mini-layout.js";
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
  // The window has one fixed size for every view (utils/mini-layout.js);
  // switching views only resets the scroll position.
  let panel: HTMLDivElement;
  let detail: DetailKind | null = null;
  const toTop = (_view: DetailKind | null) => panel?.scrollTo(0, 0);
  $: toTop(detail);
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
  // Pinned: clicking another window does not hide the mini window
  // (saved as behavior.miniPinned; the backend owns the state).
  let pinned = false;
  async function togglePin() {
    try {
      pinned = await invoke<boolean>("set_companion_pinned", {
        pinned: !pinned,
      });
    } catch (e) {
      error = String(e);
    }
  }
  // Fob: the pinned window moves by dragging its title bar (not its
  // buttons); the backend saves where it settles. Double-click: back to
  // the tray position.
  // The OS move loop starts only once the pointer moves with the button
  // held: started on the press, it swallows the release, and a double-click
  // never arrives.
  let press: { x: number; y: number } | null = null;
  const onBar = (event: MouseEvent) =>
    event.button === 0 &&
    !(event.target as HTMLElement).closest("button, a, input");
  function pressBar(event: MouseEvent) {
    press =
      pinned && onBar(event) ? { x: event.screenX, y: event.screenY } : null;
  }
  function moveBar(event: MouseEvent) {
    if (!press || (event.buttons & 1) === 0) return void (press = null);
    if (Math.hypot(event.screenX - press.x, event.screenY - press.y) < 4)
      return;
    press = null;
    void getCurrentWindow()
      .startDragging()
      .catch((e) => (error = String(e)));
  }
  function backToTray(event: MouseEvent) {
    if (!onBar(event)) return;
    void invoke("reset_mini_position").catch((e) => (error = String(e)));
  }
  // Fob: only backgrounds fade (app.css, --mini-alpha); text stays opaque.
  $: alpha = miniOpacity($settingsStore.behavior.miniOpacity) / 100;
  $: tint = miniTint($settingsStore.behavior.miniTint) / 100;
  $: boost = inkBoost(alpha * 100, tint * 100);
  $: if (typeof document !== "undefined") {
    const root = document.documentElement.style;
    root.setProperty("--mini-alpha", String(alpha));
    root.setProperty("--mini-tint", String(tint));
    root.setProperty("--ink-boost", String(boost));
    applyInk($themeStore, boost);
  }
  // Fob: text pushed toward full contrast as the window clears. The theme
  // writes --foreground on <html>; the boosted copies live on <body>, which
  // holds every view and popup portal.
  function applyInk(_theme: unknown, k: number) {
    const css = getComputedStyle(document.documentElement);
    const ink = document.documentElement.classList.contains("dark")
      ? "#fff"
      : "#000";
    const pct = (f: number) => `${Math.round(Math.min(1, k * f) * 100)}%`;
    for (const [name, f] of [
      ["foreground", 1],
      ["card-foreground", 1],
      ["popover-foreground", 1],
      ["muted-foreground", 1.4],
    ] as const) {
      const base = css.getPropertyValue(`--${name}`).trim();
      if (base)
        document.body.style.setProperty(
          `--${name}`,
          `color-mix(in oklab, ${base}, ${ink} ${pct(f)})`,
        );
    }
  }
  let opacityOpen = false;
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
    void settingsStore.init();
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
      pinned = await invoke<boolean>("get_companion_pinned").catch(() => false);
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
  <div class="mini" bind:this={panel} style={miniStyle()}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="mini-bar"
      class:movable={pinned}
      title={pinned ? $t("mini.dragHint") : $t("mini.resetHint")}
      onmousedown={pressBar}
      onmousemove={moveBar}
      onmouseup={() => (press = null)}
      ondblclick={backToTray}
    >
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
      <Popover.Root bind:open={opacityOpen}>
        <Popover.Trigger
          >{#snippet child({ props })}<Button
              {...props}
              variant="ghost"
              size="icon-sm"
              aria-label={$t("mini.opacity")}
              title={$t("mini.opacity")}><Blend /></Button
            >{/snippet}</Popover.Trigger
        >
        <Popover.Content align="end" class="w-auto p-3">
          <label for="mini-opacity-quick" class="mb-2 block text-xs font-medium"
            >{$t("mini.opacity")}</label
          >
          <OpacitySlider id="mini-opacity-quick" />
          <label
            for="mini-tint-quick"
            class="mt-3 mb-2 block text-xs font-medium">{$t("mini.tint")}</label
          >
          <OpacitySlider id="mini-tint-quick" setting="miniTint" />
        </Popover.Content>
      </Popover.Root>
      <Button
        variant={pinned ? "secondary" : "ghost"}
        size="icon-sm"
        class={pinned ? "pinned" : ""}
        aria-pressed={pinned}
        aria-label={pinned ? $t("mini.unpin") : $t("mini.pin")}
        title={pinned ? $t("mini.unpin") : $t("mini.pin")}
        onclick={togglePin}
        >{#if pinned}<Pin class="fill-current" />{:else}<PinOff />{/if}</Button
      >
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
          onSpace={() =>
            void invoke("open_main_window", {
              process: null,
              page: "space",
            }).catch(() => {})}
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
    height: 100vh;
    /* Fob: the window is transparent; this panel is its (fading) surface. */
    background: color-mix(
        in oklab,
        var(--background) calc(var(--mini-alpha, 1) * 100%),
        transparent
      )
      linear-gradient(var(--glass-tint), var(--glass-tint));
    box-shadow:
      inset 0 1px 0 var(--glass-edge),
      inset 0 0 0 1px var(--glass-rim),
      inset 0 0 32px var(--glass-glow);
    box-sizing: border-box;
    overflow-y: auto;
    /* Only long detail lists scroll; keep the bar thin and trackless. */
    scrollbar-width: thin;
    scrollbar-color: color-mix(
        in oklab,
        var(--muted-foreground) 45%,
        transparent
      )
      transparent;
    padding: var(--mini-padTop) 10px var(--mini-padBottom);
  }
  .mini-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: var(--mini-bar);
    margin-bottom: var(--mini-barGap);
  }
  .mini-bar.movable {
    cursor: move;
  }
  .mini-title {
    font-size: 13px;
    font-weight: 650;
  }
  .mini-status {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .mini-bar :global(.pinned) {
    color: var(--primary);
    background: color-mix(in oklab, var(--primary) 18%, transparent);
  }
  .mini-error {
    font-size: 12px;
    color: var(--destructive);
    margin-bottom: 6px;
  }
</style>
