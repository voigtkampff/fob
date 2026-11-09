<script lang="ts">
  import ResourceDetails from "./ResourceDetails.svelte";
  import StatCard from "./StatCard.svelte";
  import CoreGrid from "./CoreGrid.svelte";
  import ResourceTrend from "./ResourceTrend.svelte";
  import type { SystemStats, ResourceSample } from "$lib/types";
  import { Cpu } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import { cpuGroups } from "$lib/utils/cpu-groups.js";
  export let stats: SystemStats;
  export let capturedAt = 0;
  export let history: ResourceSample[] = [];
  export let isFrozen = false;
  export let unavailable = false;
  export let cpuUsage: number[];
  $: groups = cpuGroups(stats.cpu_topology, cpuUsage);
  $: average =
    cpuUsage.length && cpuUsage.every(Number.isFinite)
      ? cpuUsage.reduce((a, b) => a + b, 0) / cpuUsage.length
      : null;
</script>

<StatCard
  title="CPU"
  color="var(--chart-1)"
  badge={$t("dash.threads", { n: cpuUsage.length })}
>
  {#snippet icon()}<Cpu />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="cpu"
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      {history}
    />{/snippet}
  {#snippet value()}
    <div class="text-2xl font-semibold tracking-tight tabular-nums">
      {average === null ? "—" : average.toFixed(1)}<span
        class="ml-0.5 text-lg font-normal text-muted-foreground"
        >{average === null ? "" : "%"}</span
      >
    </div>
    <p class="text-xs leading-4 text-muted-foreground">{$t("stats.avgUtil")}</p>
  {/snippet}
  {#snippet middle()}<CoreGrid usage={cpuUsage} {groups} />{/snippet}
  {#snippet band()}<ResourceTrend
      {history}
      metric="cpu"
      label={$t("stats.cpuUtil")}
      {isFrozen}
      {unavailable}
    />{/snippet}
</StatCard>
