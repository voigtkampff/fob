<script lang="ts">
  import ResourceDetails from "./ResourceDetails.svelte";
  import StatCard from "./StatCard.svelte";
  import StatItem from "./StatItem.svelte";
  import ResourceTrend from "./ResourceTrend.svelte";
  import type { SystemStats, ResourceSample } from "$lib/types";
  import { MemoryStick } from "@lucide/svelte";
  import { formatMemorySize } from "$lib/utils";
  import { t } from "$lib/i18n";
  export let stats: SystemStats;
  export let capturedAt = 0;
  export let history: ResourceSample[] = [];
  export let isFrozen = false;
  export let unavailable = false;
  export let memoryTotal: number;
  export let memoryUsed: number;
  export let memoryFree: number;
  $: ready =
    Number.isFinite(memoryTotal) &&
    memoryTotal > 0 &&
    Number.isFinite(memoryUsed);
  $: sub = ready
    ? $t("stats.usedOf", { v: formatMemorySize(memoryTotal) })
    : $t("stats.unknownTotal");
  $: percentage = ready
    ? Math.max(0, Math.min(100, (memoryUsed / memoryTotal) * 100))
    : null;
</script>

<StatCard title={$t("dash.memory")} color="var(--chart-3)">
  {#snippet icon()}<MemoryStick />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="memory"
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      {history}
    />{/snippet}
  {#snippet value()}
    <div class="text-2xl leading-8 font-semibold tracking-tight tabular-nums">
      {ready ? formatMemorySize(memoryUsed) : "—"}
    </div>
    <p class="truncate text-xs leading-4 text-muted-foreground" title={sub}>
      {sub}
    </p>
  {/snippet}
  {#snippet middle()}
    <StatItem
      label={$t("stats.notInUse")}
      value={ready ? formatMemorySize(memoryFree) : $t("common.na")}
    />
    <StatItem
      label={$t("stats.utilization")}
      value={percentage === null
        ? $t("common.na")
        : `${percentage.toFixed(1)}%`}
    />
  {/snippet}
  {#snippet band()}<ResourceTrend
      {history}
      metric="memory"
      label={$t("stats.memUtil")}
      {isFrozen}
      {unavailable}
    />{/snippet}
</StatCard>
