<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import ResourceDetails from "./ResourceDetails.svelte";
  import StatCard from "./StatCard.svelte";
  import ResourceTrend from "./ResourceTrend.svelte";
  import type { SystemStats, GpuDevice, ResourceSample } from "$lib/types";
  import * as NativeSelect from "$lib/components/ui/native-select/index.js";
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
  $: rows = device
    ? [
        [
          $t("gpu.utilization"),
          device.utilization === null ? na : percent(device.utilization),
        ],
        [
          device.memory_kind === "shared"
            ? $t("gpu.sharedInUse")
            : $t("gpu.memInUse"),
          bytes(device.memory_used),
        ],
        [
          $t("gpu.memCapacity"),
          device.memory_kind === "shared"
            ? $t("gpu.usesSystem")
            : bytes(device.memory_total),
        ],
        [
          $t("gpu.temperature"),
          device.temperature === null
            ? na
            : `${device.temperature.toFixed(0)} °C`,
        ],
        [
          $t("gpu.power"),
          device.power_watts == null
            ? na
            : `${device.power_watts.toFixed(1)} W`,
        ],
        [
          $t("gpu.clock"),
          device.core_clock_mhz == null ? na : `${device.core_clock_mhz} MHz`,
        ],
      ]
    : [];
</script>

<StatCard title="GPU" color="var(--chart-2)">
  {#snippet icon()}<Gpu />{/snippet}
  {#snippet action()}<ResourceDetails
      kind="gpu"
      {stats}
      {capturedAt}
      {isFrozen}
      {unavailable}
      >{#snippet children()}
        {#if gpus.length > 1}<div class="space-y-2">
            <label for="gpu-device" class="text-sm font-medium"
              >{$t("gpu.adapter")}</label
            ><NativeSelect.Root
              id="gpu-device"
              value={device?.id ?? ""}
              onchange={(event) => (selectedId = event.currentTarget.value)}
              class="w-full"
              >{#each gpus as gpu}<option value={gpu.id}>{gpu.name}</option
                >{/each}</NativeSelect.Root
            >
          </div>{/if}
        {#if device}
          <div>
            <h3 class="text-lg font-semibold">{device.name}</h3>
            <p class="text-sm text-muted-foreground">
              {device.vendor}{device.cores
                ? ` · ${$t("gpu.cores", { n: device.cores })}`
                : ""}
            </p>
          </div>
          <Card.Root size="sm"
            ><Card.Content class="divide-y divide-border">
              {#each rows as [label, value]}<div
                  class="flex justify-between gap-4 py-3 first:pt-0 last:pb-0"
                >
                  <span class="text-muted-foreground">{label}</span><span
                    class="text-right font-medium tabular-nums">{value}</span
                  >
                </div>{/each}
            </Card.Content></Card.Root
          >
          <ResourceTrend
            {history}
            metric="gpu"
            expanded
            label={$t("stats.gpuUtilOf", { name: device.name })}
            gpuId={device.id}
            {isFrozen}
            {unavailable}
            unsupported={device.utilization === null}
          />
          <div
            class="rounded-lg border border-border bg-muted/40 p-4 text-sm leading-relaxed space-y-2"
          >
            <p class="font-medium">{$t("gpu.about")}</p>
            <p class="text-muted-foreground">
              {device.source}. {device.memory_kind === "shared"
                ? $t("gpu.sharedNote")
                : ""}{$t("gpu.counterNote")}
            </p>
            <p class="text-xs text-muted-foreground">
              {isFrozen ? $t("details.pausedBoth") : $t("details.historyNote")}
            </p>
          </div>
        {:else}<div class="space-y-3 py-6">
            <Gpu class="size-8 text-muted-foreground" />
            <h3 class="font-semibold">{$t("gpu.unavailable")}</h3>
            <p class="text-sm leading-relaxed text-muted-foreground">
              {$t("gpu.unavailableNote")}
            </p>
          </div>{/if}
      {/snippet}</ResourceDetails
    >{/snippet}
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
