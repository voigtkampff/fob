<script lang="ts">
  // Fob: the Temp & fan card of the process tab (same sensor source as the
  // dashboard card); its arrow opens the Temp & fan side panel.
  import StatCard from "./StatCard.svelte";
  import StatItem from "./StatItem.svelte";
  import ResourceTrend from "./ResourceTrend.svelte";
  import type { ResourceSample, Sensor, SystemStats } from "$lib/types";
  import ResourceDetails from "./ResourceDetails.svelte";
  import { Thermometer } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    sensorSummary,
    fanText,
    formatSensor,
    setupState,
  } from "$lib/utils/dashboard.js";
  import { sensorSetup, refreshSensorSetup } from "$lib/stores/sensorSetup";
  import { onMount } from "svelte";
  import { t } from "$lib/i18n";
  export let stats: SystemStats;
  export let capturedAt = 0;
  export let sensors: Sensor[] = [];
  export let sources: string[] = [];
  export let history: ResourceSample[] = [];
  export let isFrozen = false;
  export let unavailable = false;
  let open = false;
  $: summary = sensorSummary(sensors);
  $: fans = fanText(sensors);
  $: hasBoard = sensors.some((s) => s.source !== "NVML");
  $: setup = setupState($sensorSetup);
  $: offerSetup =
    !hasBoard &&
    ["enable", "repair", "update", "missing-files"].includes(setup);
  $: head = summary.cpuTemp ?? summary.gpuTemp;
  $: sub = head
    ? `${summary.cpuTemp ? "CPU" : "GPU"} · ${head.name}`
    : $t("stats.noTemp");
  onMount(() => void refreshSensorSetup());
</script>

<StatCard title={$t("dash.temp")} color="var(--chart-5, var(--chart-2))">
  {#snippet icon()}<Thermometer />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="temp"
      bind:open
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      {history}
      {sensors}
      {sources}
    />{/snippet}
  {#snippet value()}
    <div class="text-2xl leading-8 font-semibold tracking-tight tabular-nums">
      {head ? head.value.toFixed(0) : "—"}<span
        class="ml-0.5 text-base font-normal text-muted-foreground"
        >{head ? "°C" : ""}</span
      >
    </div>
    <p class="truncate text-xs leading-4 text-muted-foreground" title={sub}>
      {sub}
    </p>
  {/snippet}
  {#snippet middle()}
    {#if offerSetup}
      <Button
        variant="outline"
        size="sm"
        class="h-auto min-h-7 w-full py-1 text-xs leading-4 whitespace-normal"
        onclick={() => (open = true)}
        >{setup === "update"
          ? $t("dash.updateHelper")
          : setup === "repair"
            ? $t("dash.repair")
            : $t("dash.enable")}</Button
      >
    {:else}
      {#if summary.cpuTemp && summary.gpuTemp}<StatItem
          label="GPU"
          value={formatSensor(summary.gpuTemp)}
        />{/if}
      <StatItem
        label={$t("stats.fans")}
        value={fans ||
          (summary.fan ? formatSensor(summary.fan) : $t("common.na"))}
      />
    {/if}
  {/snippet}
  {#snippet band()}<ResourceTrend
      {history}
      metric="temp"
      label={$t("dash.temp")}
      {isFrozen}
      {unavailable}
      unsupported={!head}
    />{/snippet}
</StatCard>
