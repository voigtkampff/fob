<script lang="ts">
  import StatCard from "./StatCard.svelte";
  import StatItem from "./StatItem.svelte";
  import ResourceDetails from "./ResourceDetails.svelte";
  import { HardDrive } from "@lucide/svelte";
  import { Progress } from "$lib/components/ui/progress/index.js";
  import { formatBytes } from "$lib/utils";
  import { t } from "$lib/i18n";
  import type { SystemStats, ResourceSample } from "$lib/types";
  export let stats: SystemStats;
  export let capturedAt = 0;
  export let history: ResourceSample[] = [];
  export let isFrozen = false;
  export let unavailable = false;
  export let diskTotalBytes: number;
  export let diskUsedBytes: number;
  export let diskFreeBytes: number;
  /** The panel's space map button. */
  export let onSpace: () => void = () => {};
  /** The health link: opens the full Disk detail. */
  export let onDiskHealth: () => void = () => {};
  $: ready = Number.isFinite(diskTotalBytes) && diskTotalBytes > 0;
  $: sub = ready
    ? $t("stats.freeOf", { v: formatBytes(diskTotalBytes) })
    : $t("stats.unknownCapacity");
  $: percentage = ready
    ? Math.max(0, Math.min(100, (diskUsedBytes / diskTotalBytes) * 100))
    : null;
</script>

<StatCard title={$t("stats.storage")} color="var(--chart-4)">
  {#snippet icon()}<HardDrive />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="storage"
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      {history}
      {onSpace}
      {onDiskHealth}
    />{/snippet}
  {#snippet value()}
    <div class="text-2xl font-semibold tracking-tight tabular-nums">
      {ready ? formatBytes(diskFreeBytes) : "—"}
    </div>
    <p class="truncate text-xs leading-4 text-muted-foreground" title={sub}>
      {sub}
    </p>
  {/snippet}
  {#snippet middle()}
    <StatItem
      label={$t("stats.used")}
      value={ready ? formatBytes(diskUsedBytes) : $t("common.na")}
    />
  {/snippet}
  {#snippet band()}
    <div class="space-y-1">
      <Progress
        value={percentage}
        class="h-2"
        style="--primary:var(--chart-4)"
        aria-label={$t("stats.storageUsedAria")}
      />
      <div
        class="flex min-h-3 justify-between gap-2 text-[10px] leading-3 text-muted-foreground tabular-nums"
      >
        <span
          >{percentage === null
            ? $t("common.na")
            : $t("stats.pctUsed", { v: percentage.toFixed(1) })}</span
        ><span
          >{percentage === null
            ? ""
            : $t("stats.pctFree", { v: (100 - percentage).toFixed(1) })}</span
        >
      </div>
    </div>
  {/snippet}
</StatCard>
