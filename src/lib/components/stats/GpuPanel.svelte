<script lang="ts">
  import ResourceDetails from "./ResourceDetails.svelte";
  import StatCard from "./StatCard.svelte";
  import ResourceTrend from "./ResourceTrend.svelte";
  import type { SystemStats, GpuDevice, ResourceSample } from "$lib/types";
  import { Gpu } from "@lucide/svelte";
  import { formatBytes } from "$lib/utils";
  import { t } from "$lib/i18n";
  export let stats: SystemStats;
  export let capturedAt = 0;
  export let gpus: GpuDevice[] = [];
  export let history: ResourceSample[] = [];
  export let isFrozen = false;
  export let unavailable = false;
  let selectedId = "";
  function reconcile(devices: GpuDevice[]) {
    if (!devices.some((g) => g.id === selectedId))
      selectedId =
        devices.find((g) => g.utilization !== null)?.id ?? devices[0]?.id ?? "";
  }
  $: reconcile(gpus);
  $: device =
    gpus.find((g) => g.id === selectedId) ??
    gpus.find((g) => g.utilization !== null) ??
    gpus[0];
  const percent = (v: number | null | undefined) =>
    v == null ? "—" : `${v.toFixed(1)}%`;
  $: na = $t("common.na");
  $: bytes = (v: number | null | undefined) =>
    v == null ? na : formatBytes(v);
</script>

<StatCard title="GPU" color="var(--chart-2)">
  {#snippet icon()}<Gpu />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="gpu"
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      {history}
      {gpus}
      gpuId={device?.id ?? ""}
      onGpu={(id) => (selectedId = id)}
    />{/snippet}
  {#snippet value()}
    <div class="text-2xl leading-8 font-semibold tracking-tight tabular-nums">
      {percent(device?.utilization)}
    </div>
    <p
      class="truncate text-xs leading-4 text-muted-foreground"
      title={device?.name}
    >
      {device?.name ?? $t("dash.noGpu")}
    </p>
  {/snippet}
  {#snippet middle()}
    <p class="text-xs leading-4 text-muted-foreground">
      {#if device?.memory_used != null}{device.memory_kind === "shared"
          ? $t("gpu.shared")
          : $t("gpu.memory")} ·
        <span class="font-medium text-foreground"
          >{formatBytes(device.memory_used)}</span
        >{:else}{$t("gpu.countersVary")}{/if}
    </p>
  {/snippet}
  {#snippet band()}<ResourceTrend
      {history}
      metric="gpu"
      label={$t("stats.gpuUtilOf", { name: device?.name ?? "GPU" })}
      gpuId={device?.id}
      {isFrozen}
      {unavailable}
      unsupported={device?.utilization == null}
    />{/snippet}
</StatCard>
