<script lang="ts">
  // Fob: the mini window opacity slider, in Settings and in the mini
  // window's title bar popover. Both write behavior.miniOpacity; every
  // window follows the saved value (preferences-change).
  import { settingsStore } from "$lib/stores/settings";
  import {
    miniOpacity,
    OPACITY_MIN,
    OPACITY_MAX,
    OPACITY_STEP,
  } from "$lib/utils/mini-opacity.js";
  import { t } from "$lib/i18n";

  export let id = "mini-opacity";
  export let describedby: string | undefined = undefined;
  /** Which setting it writes (both are whole percents, 0–100). */
  export let key: "miniOpacity" | "overlayOpacity" = "miniOpacity";
  export let fallback = 100;
  $: saved = $settingsStore.behavior[key];
  $: value = saved == null ? fallback : miniOpacity(saved);
  function set(next: number) {
    const v = miniOpacity(next);
    if (v === value) return;
    settingsStore.updateConfig({
      behavior: { ...$settingsStore.behavior, [key]: v },
    });
  }
</script>

<div class="flex items-center gap-3">
  <input
    {id}
    type="range"
    class="fob-range w-40 cursor-pointer"
    style:--fill="{value}%"
    min={OPACITY_MIN}
    max={OPACITY_MAX}
    step={OPACITY_STEP}
    {value}
    aria-describedby={describedby}
    aria-valuetext={$t("opacity.value", { v: value })}
    oninput={(e) => set(Number(e.currentTarget.value))}
  />
  <span class="w-10 text-right text-xs tabular-nums text-muted-foreground"
    >{value}%</span
  >
</div>

<style>
  /* Fob: track fill and thumb follow the theme accent (--primary). */
  .fob-range {
    appearance: none;
    height: 16px;
    background: transparent;
  }
  .fob-range::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--primary) var(--fill),
      color-mix(in oklab, var(--foreground) 16%, transparent) var(--fill)
    );
  }
  .fob-range::-webkit-slider-thumb {
    appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -4px;
    border-radius: 50%;
    background: var(--primary);
    border: 2px solid var(--primary-foreground);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.35);
  }
  .fob-range::-moz-range-track {
    height: 6px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 16%, transparent);
  }
  .fob-range::-moz-range-progress {
    height: 6px;
    border-radius: 999px;
    background: var(--primary);
  }
  .fob-range::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--primary);
    border: 2px solid var(--primary-foreground);
  }
  .fob-range:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
    border-radius: 4px;
  }
</style>
