<script lang="ts">
  // Fob: uptime from the same source as the dashboard and the mini window
  // (the last real power-on from the boot log, else the kernel uptime).
  // Load average is gone: Windows has none.
  import ResourceDetails from "./ResourceDetails.svelte";
  import StatCard from "./StatCard.svelte";
  import StatItem from "./StatItem.svelte";
  import type { SystemStats } from "$lib/types";
  import { Server } from "@lucide/svelte";
  import { formatDuration, uptimeInfo } from "$lib/utils/dashboard.js";
  import { clockTime } from "$lib/utils/chart-cursor.js";
  import { t } from "$lib/i18n";
  export let stats: SystemStats;
  export let capturedAt = 0;
  export let isFrozen = false;
  export let unavailable = false;
  /** [Unix seconds, kind] of the last power-on (Extra.started). */
  export let started: [number, string] | null = null;
  $: since = uptimeInfo(
    stats.uptime,
    started?.[0],
    (capturedAt || Date.now()) / 1000,
  );
  $: bootedAt = started
    ? new Date(started[0] * 1000)
    : new Date((capturedAt || Date.now()) - stats.uptime * 1000);
</script>

<StatCard title={$t("dash.system")}>
  {#snippet icon()}<Server />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="system"
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      {started}
    />{/snippet}
  {#snippet value()}
    <div
      class="truncate text-xl leading-8 font-semibold tracking-tight tabular-nums"
    >
      {Number.isFinite(since.seconds) ? formatDuration(since.seconds, $t) : "—"}
    </div>
    <p class="truncate text-xs leading-4 text-muted-foreground">
      {since.sinceStart && started
        ? $t("stats.sincePowerOn", { how: $t(`uptime.kind.${started[1]}`) })
        : $t("stats.kernelUptime")}
    </p>
  {/snippet}
  {#snippet middle()}
    {#if stats.details.identity.os_version}<StatItem
        label={$t("stats.os")}
        value={stats.details.identity.os_version}
      />{/if}
    {#if since.sinceStart}<StatItem
        label={$t("stats.kernel")}
        value={formatDuration(since.kernel, $t)}
      />{/if}
  {/snippet}
  {#snippet band()}
    <div
      class="min-h-3 truncate text-[10px] leading-3 text-muted-foreground"
      title={`${bootedAt.toLocaleDateString()} ${clockTime(bootedAt.getTime())}`}
    >
      {$t("stats.bootedAt", {
        d: bootedAt.toLocaleString(undefined, {
          month: "numeric",
          day: "numeric",
          hour: "2-digit",
          minute: "2-digit",
          hourCycle: "h23",
        }),
      })}
    </div>
  {/snippet}
</StatCard>
