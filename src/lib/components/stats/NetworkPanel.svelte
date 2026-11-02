<script lang="ts">
  import ResourceDetails from "./ResourceDetails.svelte";
  import StatCard from "./StatCard.svelte";
  import StatItem from "./StatItem.svelte";
  import ResourceTrend from "./ResourceTrend.svelte";
  import type { SystemStats, ResourceSample } from "$lib/types";
  import { Network } from "@lucide/svelte";
  import { formatBytes } from "$lib/utils";
  import { t } from "$lib/i18n";
  export let stats: SystemStats;
  export let capturedAt = 0;
  export let history: ResourceSample[] = [];
  export let isFrozen = false;
  export let unavailable = false;
  export let networkRxBytes: number | null;
  export let networkTxBytes: number | null;
</script>

<StatCard title={$t("dash.network")} color="var(--chart-1)">
  {#snippet icon()}<Network />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="network"
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      {history}
    />{/snippet}
  {#snippet value()}
    <div
      class="truncate text-2xl leading-8 font-semibold tracking-tight tabular-nums"
    >
      {networkRxBytes !== null && Number.isFinite(networkRxBytes)
        ? formatBytes(networkRxBytes)
        : "—"}<span class="ml-0.5 text-base font-normal text-muted-foreground"
        >/s</span
      >
    </div>
    <p
      class="flex items-center gap-1.5 text-xs leading-4 text-muted-foreground"
    >
      <span class="w-2.5 border-t-2 border-[var(--chart-1)]" aria-hidden="true"
      ></span>↓ {$t("stats.received")}
    </p>
  {/snippet}
  {#snippet middle()}
    <div class="flex items-center gap-1.5">
      <span
        class="w-2.5 shrink-0 border-t-2 border-dashed border-[var(--chart-4)]"
        aria-hidden="true"
      ></span>
      <div class="min-w-0 flex-1">
        <StatItem
          label={`↑ ${$t("stats.sent")}`}
          value={networkTxBytes !== null && Number.isFinite(networkTxBytes)
            ? `${formatBytes(networkTxBytes)}/s`
            : $t("common.na")}
        />
      </div>
    </div>
  {/snippet}
  {#snippet band()}<ResourceTrend
      {history}
      metric="network"
      label={$t("stats.netTrend")}
      {isFrozen}
      {unavailable}
    />{/snippet}
</StatCard>
