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
  $: value = miniOpacity($settingsStore.behavior.miniOpacity);
  function set(next: number) {
    const v = miniOpacity(next);
    if (v === value) return;
    settingsStore.updateConfig({
      behavior: { ...$settingsStore.behavior, miniOpacity: v },
    });
  }
</script>

<div class="flex items-center gap-3">
  <input
    {id}
    type="range"
    class="h-1.5 w-40 cursor-pointer accent-primary"
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
