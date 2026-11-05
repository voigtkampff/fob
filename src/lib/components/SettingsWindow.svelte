<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import { settingsStore, settingsStatus } from "$lib/stores/settings";
  import { REFRESH_RATE_OPTIONS } from "$lib/constants";
  import ThemeGallery from "./ThemeGallery.svelte";
  import HotkeyField from "./HotkeyField.svelte";
  import OpacitySlider from "./OpacitySlider.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as NativeSelect from "$lib/components/ui/native-select/index.js";
  import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
  import {
    Settings2,
    Palette,
    ChevronUp,
    ChevronDown,
    GripVertical,
    ChartSpline,
  } from "@lucide/svelte";
  import {
    overlaySettings,
    settingsRows,
    toggleMetric,
    moveMetric,
    reorderMetric,
    CHART_METRICS,
    chartsPatch,
    toggleChart,
    DOCKS,
    OPACITY_FLOOR,
    DEFAULT_OPACITY,
    type MetricId,
  } from "$lib/utils/overlay.js";
  import { t } from "$lib/i18n";
  let pane = "general";
  let runtimeError = "";
  // Fob: the mini window has a dragged position (reset button enabled).
  let dragged = false;
  const readDragged = () =>
    invoke<boolean>("get_mini_position_saved")
      .then((v) => (dragged = v))
      .catch(() => {});
  async function backToTray() {
    await invoke("reset_mini_position").catch(() => {});
    await readDragged();
  }
  $: rate = $settingsStore.behavior.refreshRate;
  $: options = REFRESH_RATE_OPTIONS.some((o) => o.value === rate)
    ? REFRESH_RATE_OPTIONS
    : [{ value: rate, label: `${rate / 1000}s` }, ...REFRESH_RATE_OPTIONS];
  function select(value: string) {
    if (!["general", "appearance"].includes(value)) return;
    pane = value;
    try {
      localStorage.setItem("settings-pane", value);
    } catch {
      /* Session selection remains available. */
    }
  }
  function save(
    key:
      | "refreshRate"
      | "menuBarEnabled"
      | "keepRunning"
      | "startInTray"
      | "language",
    value: number | boolean | string,
  ) {
    settingsStore.updateConfig({
      behavior: { ...$settingsStore.behavior, [key]: value },
    });
  }
  $: overlay = overlaySettings($settingsStore.behavior);
  // Drag and drop reorder: the shown row being dragged, and the slot (0 to
  // shown.length) it would land in.
  let dragFrom = -1;
  let dropAt = -1;
  function dragOver(event: DragEvent, i: number, on: boolean) {
    if (dragFrom < 0) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    if (!on) {
      dropAt = overlay.metrics.length;
      return;
    }
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    dropAt = event.clientY < box.top + box.height / 2 ? i : i + 1;
  }
  function drop(event: DragEvent) {
    event.preventDefault();
    if (dragFrom >= 0 && dropAt >= 0)
      setMetrics(reorderMetric(overlay.metrics, dragFrom, dropAt));
    dragFrom = dropAt = -1;
  }
  function saveOverlay(patch: Record<string, unknown>) {
    settingsStore.updateConfig({
      behavior: { ...$settingsStore.behavior, ...patch },
    });
  }
  const setMetrics = (metrics: MetricId[]) =>
    saveOverlay({ overlayMetrics: metrics });
  onMount(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    void (async () => {
      const stopPane = await listen<string>("settings-pane", ({ payload }) =>
        select(payload),
      );
      const stopError = await listen<string>(
        "preferences-runtime-error",
        ({ payload }) => {
          runtimeError = payload;
        },
      );
      if (disposed) {
        stopPane();
        stopError();
        return;
      }
      stops.push(stopPane, stopError);
      select(
        (await invoke<string>("settings_pane")) ||
          localStorage.getItem("settings-pane") ||
          "general",
      );
      await settingsStore.init();
      await readDragged();
      await emit("preferences-runtime-status-request");
    })();
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  });
</script>

<svelte:window onfocus={readDragged} />

<div class="flex h-screen flex-col bg-background">
  <header
    class="flex shrink-0 justify-center border-b border-border bg-card px-6 py-4"
  >
    <ToggleGroup.Root
      type="single"
      value={pane}
      onValueChange={select}
      variant="outline"
      aria-label={$t("settings.section")}
    >
      <ToggleGroup.Item value="general" class="px-5"
        ><Settings2 />{$t("settings.general")}</ToggleGroup.Item
      >
      <ToggleGroup.Item value="appearance" class="px-5"
        ><Palette />{$t("settings.appearance")}</ToggleGroup.Item
      >
    </ToggleGroup.Root>
  </header>
  <div class="min-h-0 flex-1 overflow-y-auto p-6">
    {#if pane === "general"}
      <h1 class="text-lg font-semibold tracking-tight">
        {$t("settings.general")}
      </h1>
      <p class="mt-1 mb-5 text-sm text-muted-foreground">
        {$t("settings.generalLead")}
      </p>
      <fieldset disabled={!$settingsStatus.ready} class="space-y-5">
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("settings.monitoring")}</Card.Title></Card.Header
          ><Card.Content>
            <div class="flex items-center justify-between gap-6">
              <div>
                <Label for="refresh-interval">{$t("settings.refresh")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.refreshNote")}
                </p>
              </div>
              <NativeSelect.Root
                id="refresh-interval"
                class="w-28"
                value={rate}
                onchange={(event) =>
                  save("refreshRate", Number(event.currentTarget.value))}
              >
                {#each options as option}<option value={option.value}
                    >{option.label}</option
                  >{/each}
              </NativeSelect.Root>
            </div>
            <p class="mt-1 text-xs text-muted-foreground">
              {$t("settings.refreshMore")}
            </p>
          </Card.Content></Card.Root
        >
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("settings.language")}</Card.Title></Card.Header
          ><Card.Content>
            <div class="flex items-center justify-between gap-6">
              <div>
                <Label for="language">{$t("settings.languageLabel")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.languageNote")}
                </p>
              </div>
              <NativeSelect.Root
                id="language"
                class="w-44"
                value={$settingsStore.behavior.language ?? "auto"}
                onchange={(event) =>
                  save("language", event.currentTarget.value)}
              >
                {#each ["auto", "en", "zh"] as option}<option value={option}
                    >{$t(`lang.${option}`)}</option
                  >{/each}
              </NativeSelect.Root>
            </div>
          </Card.Content></Card.Root
        >
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("settings.tray")}</Card.Title></Card.Header
          ><Card.Content class="gap-5">
            <div class="flex items-start gap-3">
              <Checkbox
                id="show-menu-icon"
                class="mt-0.5"
                checked={$settingsStore.behavior.menuBarEnabled}
                onCheckedChange={(value) => save("menuBarEnabled", value)}
              />
              <div>
                <Label for="show-menu-icon">{$t("settings.showTray")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.showTrayNote")}
                </p>
              </div>
            </div>
            <div class="flex items-start gap-3">
              <Checkbox
                id="start-in-tray"
                class="mt-0.5"
                disabled={!$settingsStore.behavior.menuBarEnabled}
                checked={$settingsStore.behavior.startInTray}
                onCheckedChange={(value) => save("startInTray", value)}
              />
              <div>
                <Label for="start-in-tray">{$t("settings.startInTray")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.startInTrayNote")}
                </p>
              </div>
            </div>
            <div class="flex items-start justify-between gap-6">
              <div>
                <Label for="mini-opacity">{$t("settings.miniOpacity")}</Label>
                <p
                  id="mini-opacity-note"
                  class="mt-1 text-xs leading-relaxed text-muted-foreground"
                >
                  {$t("settings.miniOpacityNote")}
                </p>
              </div>
              <div class="shrink-0 pt-1">
                <OpacitySlider describedby="mini-opacity-note" />
              </div>
            </div>
            <div class="flex items-start justify-between gap-6">
              <div>
                <p class="text-sm leading-none font-medium">
                  {$t("settings.miniPosition")}
                </p>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.miniPositionNote")}
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                class="shrink-0"
                disabled={!dragged}
                onclick={backToTray}>{$t("settings.resetPosition")}</Button
              >
            </div>
            <p class="text-xs leading-relaxed text-muted-foreground">
              {$t("settings.closeNote")}
            </p>
          </Card.Content></Card.Root
        >
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("settings.overlay")}</Card.Title></Card.Header
          ><Card.Content class="gap-5">
            <p class="text-xs leading-relaxed text-muted-foreground">
              {$t("settings.overlayLead")}
            </p>
            <div class="flex items-start gap-3">
              <Checkbox
                id="overlay-enabled"
                class="mt-0.5"
                checked={overlay.enabled}
                onCheckedChange={(value) =>
                  saveOverlay({ overlayEnabled: value })}
              />
              <div>
                <Label for="overlay-enabled"
                  >{$t("settings.overlayEnable")}</Label
                >
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.overlayEnableNote")}
                </p>
              </div>
            </div>
            <div>
              <p class="text-sm leading-none font-medium">
                {$t("settings.overlayMetrics")}
              </p>
              <p
                class="mt-1 mb-2 text-xs leading-relaxed text-muted-foreground"
              >
                {$t("settings.overlayMetricsNote")}
              </p>
              <ul class="overlay-metrics">
                {#each settingsRows(overlay.metrics) as row, i (row.id)}
                  {@const name = $t(`overlay.metric.${row.id}`)}
                  {@const charted = overlay.charts.includes(row.id)}
                  {@const chartable = CHART_METRICS.includes(row.id)}
                  <li
                    class:dragging={row.on && dragFrom === i}
                    class:drop-before={dragFrom >= 0 && dropAt === i}
                    class:drop-after={dragFrom >= 0 &&
                      row.on &&
                      dropAt === overlay.metrics.length &&
                      i === overlay.metrics.length - 1}
                    draggable={row.on}
                    ondragstart={(e) => {
                      if (!row.on) return;
                      dragFrom = i;
                      e.dataTransfer?.setData("text/plain", row.id);
                      if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
                    }}
                    ondragover={(e) => dragOver(e, i, row.on)}
                    ondrop={drop}
                    ondragend={() => (dragFrom = dropAt = -1)}
                  >
                    <span
                      class="grip"
                      class:off={!row.on}
                      title={row.on ? $t("settings.overlayDrag") : undefined}
                      aria-hidden="true"><GripVertical class="size-3.5" /></span
                    >
                    <Checkbox
                      id={`overlay-m-${row.id}`}
                      checked={row.on}
                      onCheckedChange={(value) =>
                        setMetrics(
                          toggleMetric(overlay.metrics, row.id, value),
                        )}
                    />
                    <button
                      type="button"
                      class="chart-toggle"
                      class:on={charted}
                      disabled={!chartable}
                      aria-pressed={charted}
                      aria-label={chartable
                        ? $t("settings.overlayChartFor", { name })
                        : $t("settings.overlayNoChart", { name })}
                      title={chartable
                        ? $t("settings.overlayChartFor", { name })
                        : $t("settings.overlayNoChart", { name })}
                      onclick={() =>
                        saveOverlay(
                          chartsPatch(
                            toggleChart(overlay.charts, row.id, !charted),
                          ),
                        )}><ChartSpline class="size-3.5" /></button
                    >
                    <Label for={`overlay-m-${row.id}`} class="flex-1"
                      >{name}</Label
                    >
                    {#if row.on}
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        disabled={i === 0}
                        aria-label={$t("settings.overlayUp", { name })}
                        title={$t("settings.overlayUp", { name })}
                        onclick={() =>
                          setMetrics(moveMetric(overlay.metrics, row.id, -1))}
                        ><ChevronUp /></Button
                      ><Button
                        variant="ghost"
                        size="icon-sm"
                        disabled={i === overlay.metrics.length - 1}
                        aria-label={$t("settings.overlayDown", { name })}
                        title={$t("settings.overlayDown", { name })}
                        onclick={() =>
                          setMetrics(moveMetric(overlay.metrics, row.id, 1))}
                        ><ChevronDown /></Button
                      >
                    {/if}
                  </li>
                {/each}
              </ul>
            </div>
            <div class="flex items-start justify-between gap-6">
              <div>
                <p class="text-sm leading-none font-medium">
                  {$t("settings.overlayDock")}
                </p>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.overlayDockNote")}
                </p>
              </div>
              <ToggleGroup.Root
                type="single"
                variant="outline"
                size="sm"
                class="shrink-0"
                value={overlay.dock}
                aria-label={$t("settings.overlayDock")}
                onValueChange={(value) =>
                  value && saveOverlay({ overlayDock: value })}
              >
                {#each DOCKS as dock}<ToggleGroup.Item value={dock} class="px-3"
                    >{$t(`settings.overlayDock.${dock}`)}</ToggleGroup.Item
                  >{/each}
              </ToggleGroup.Root>
            </div>
            <div class="flex items-start justify-between gap-6">
              <div>
                <Label for="overlay-opacity"
                  >{$t("settings.overlayOpacity")}</Label
                >
                <p
                  id="overlay-opacity-note"
                  class="mt-1 text-xs leading-relaxed text-muted-foreground"
                >
                  {$t("settings.overlayOpacityNote")}
                </p>
              </div>
              <div class="shrink-0 pt-1">
                <OpacitySlider
                  id="overlay-opacity"
                  key="overlayOpacity"
                  fallback={DEFAULT_OPACITY}
                  min={OPACITY_FLOOR}
                  describedby="overlay-opacity-note"
                />
              </div>
            </div>
            <div class="flex items-start gap-3">
              <Checkbox
                id="overlay-unlocked"
                class="mt-0.5"
                checked={overlay.unlocked}
                onCheckedChange={(value) =>
                  saveOverlay({ overlayUnlocked: value })}
              />
              <div>
                <Label for="overlay-unlocked"
                  >{$t("settings.overlayUnlock")}</Label
                >
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.overlayUnlockNote")}
                </p>
              </div>
            </div>
          </Card.Content></Card.Root
        >
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("hotkey.title")}</Card.Title></Card.Header
          ><Card.Content>
            <div class="flex items-start justify-between gap-6">
              <div>
                <Label for="mini-hotkey">{$t("hotkey.label")}</Label>
                <p
                  id="mini-hotkey-note"
                  class="mt-1 text-xs leading-relaxed text-muted-foreground"
                >
                  {$t("hotkey.note")}
                </p>
              </div>
              <div class="shrink-0"><HotkeyField /></div>
            </div>
            <div class="mt-4 flex items-start justify-between gap-6">
              <div>
                <Label for="overlay-hotkey">{$t("hotkey.overlayLabel")}</Label>
                <p
                  id="overlay-hotkey-note"
                  class="mt-1 text-xs leading-relaxed text-muted-foreground"
                >
                  {$t("hotkey.overlayNote")}
                </p>
              </div>
              <div class="shrink-0">
                <HotkeyField
                  target="overlay"
                  id="overlay-hotkey"
                  describedby="overlay-hotkey-note"
                  label={$t("hotkey.overlayAria")}
                />
              </div>
            </div>
          </Card.Content></Card.Root
        >
      </fieldset>
    {:else}
      <h1 class="text-lg font-semibold tracking-tight">
        {$t("settings.appearance")}
      </h1>
      <p class="mt-1 mb-5 text-sm text-muted-foreground">
        {$t("settings.appearanceLead")}
      </p>
      <ThemeGallery embedded />
    {/if}
  </div>
  <footer class="shrink-0 border-t border-border bg-card px-6 py-3">
    {#if $settingsStatus.error || runtimeError}
      <div
        role="alert"
        class="flex items-center justify-between gap-3 text-xs text-destructive"
      >
        <span>{$settingsStatus.error || runtimeError}</span><Button
          variant="outline"
          size="sm"
          onclick={() => {
            void settingsStore.retry();
            void emit("preferences-runtime-retry");
          }}>{$t("settings.retry")}</Button
        >
      </div>
    {:else}<p role="status" class="text-xs text-muted-foreground">
        {!$settingsStatus.ready
          ? $t("settings.loading")
          : $settingsStatus.saving
            ? $t("settings.saving")
            : $t("settings.saved")}
      </p>{/if}
  </footer>
</div>

<style>
  .overlay-metrics {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .overlay-metrics li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 28px;
    border-radius: 6px;
    box-shadow: none;
    border-top: 2px solid transparent;
    border-bottom: 2px solid transparent;
  }
  .overlay-metrics li[draggable="true"] {
    cursor: grab;
  }
  .overlay-metrics li.dragging {
    opacity: 0.45;
  }
  .overlay-metrics li.drop-before {
    border-top-color: var(--primary);
  }
  .overlay-metrics li.drop-after {
    border-bottom-color: var(--primary);
  }
  .chart-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    border: 1px solid transparent;
    color: var(--muted-foreground);
    opacity: 0.6;
  }
  .chart-toggle:hover:not(:disabled) {
    background: var(--accent);
    opacity: 1;
  }
  .chart-toggle.on {
    color: var(--primary);
    border-color: color-mix(in oklab, var(--primary), transparent 60%);
    background: color-mix(in oklab, var(--primary), transparent 88%);
    opacity: 1;
  }
  .chart-toggle:disabled {
    opacity: 0.25;
    cursor: not-allowed;
  }
  .grip {
    display: inline-flex;
    color: var(--muted-foreground);
  }
  .grip.off {
    visibility: hidden;
  }
</style>
