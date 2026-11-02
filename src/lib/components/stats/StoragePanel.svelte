<script lang="ts">
  import StatCard from "./StatCard.svelte";
  import StatItem from "./StatItem.svelte";
  import { HardDrive, ArrowUpRight } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Progress } from "$lib/components/ui/progress/index.js";
  import { formatBytes } from "$lib/utils";
  import { t } from "$lib/i18n";
  export let diskTotalBytes: number;
  export let diskUsedBytes: number;
  export let diskFreeBytes: number;
  /** The arrow opens the disk space map. */
  export let onOpen: () => void = () => {};
  $: ready = Number.isFinite(diskTotalBytes) && diskTotalBytes > 0;
  $: percentage = ready
    ? Math.max(0, Math.min(100, (diskUsedBytes / diskTotalBytes) * 100))
    : null;
</script>

<StatCard title={$t("stats.storage")} color="var(--chart-4)">
  {#snippet icon()}<HardDrive />{/snippet}
  {#snippet action()}<Button
      variant="ghost"
      size="icon-xs"
      aria-label={$t("space.open")}
      title={$t("space.open")}
      onclick={onOpen}><ArrowUpRight class="size-3.5" /></Button
    >{/snippet}
  {#snippet value()}
    <div class="text-2xl leading-8 font-semibold tracking-tight tabular-nums">
      {ready ? formatBytes(diskFreeBytes) : "—"}
    </div>
    <p class="truncate text-xs leading-4 text-muted-foreground">
      {ready
        ? $t("stats.freeOf", { v: formatBytes(diskTotalBytes) })
        : $t("stats.unknownCapacity")}
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
