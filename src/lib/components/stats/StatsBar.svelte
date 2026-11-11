<script lang="ts">
  import GpuPanel from "./GpuPanel.svelte";
  import TempPanel from "./TempPanel.svelte";
  import { t } from "$lib/i18n";
  import { setContext } from "svelte";
  import { dragReorder } from "$lib/utils/drag-reorder.js";
  import {
    PROCESS_CARDS,
    moveNextTo,
    previewOrder,
    stepOrder,
  } from "$lib/utils/layout-order.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import type {
    SystemStats,
    ResourceSample,
    GpuDevice,
    Sensor,
  } from "$lib/types";
  import {
    CpuPanel,
    MemoryPanel,
    StoragePanel,
    SystemPanel,
    NetworkPanel,
  } from "$lib/components";
  export let systemStats: SystemStats | null = null;
  export let history: ResourceSample[] = [];
  export let gpus: GpuDevice[] = [];
  export let capturedAt = 0;
  export let isFrozen = false;
  export let unavailable = false;
  export let sensors: Sensor[] = [];
  export let started: [number, string] | null = null;
  export let sources: string[] = [];
  export let onOpenSpace: () => void = () => {};
  export let onOpenDiskHealth: () => void = () => {};
  export let order: string[] = PROCESS_CARDS;
  export let onReorder: ((order: string[]) => void) | null = null;
  let move: { id: string; target: string; after: boolean } | null = null;
  // StatCard shows the drag grip and hint only when its cards can move.
  setContext("movable-cards", !!onReorder);
  $: shown = previewOrder(order, move);
</script>

{#if systemStats}<Tooltip.Provider delayDuration={150}
    ><section
      class="resource-grid shrink-0"
      class:movable={!!onReorder}
      aria-label={$t("stats.aria")}
      use:dragReorder={{
        ids: shown.map((id) => (onReorder ? id : null)),
        handle: ".sc-title",
        vertical: true,
        onMove: (id, target, after) =>
          onReorder?.(moveNextTo(order, id, target, after)),
        onPreview: (next) => (move = next),
        onStep: (id, step) => onReorder?.(stepOrder(order, id, step)),
      }}
    >
      {#each shown as id (id)}
        {#if id === "cpu"}
          <CpuPanel
            stats={systemStats}
            {capturedAt}
            cpuUsage={systemStats.cpu_usage}
            {history}
            {isFrozen}
            {unavailable}
          />
        {:else if id === "memory"}
          <MemoryPanel
            stats={systemStats}
            {capturedAt}
            {history}
            {isFrozen}
            {unavailable}
            memoryTotal={systemStats.memory_total}
            memoryUsed={systemStats.memory_used}
            memoryFree={systemStats.memory_free}
          />
        {:else if id === "gpu"}
          <GpuPanel
            stats={systemStats}
            {capturedAt}
            {gpus}
            {history}
            {isFrozen}
            {unavailable}
          />
        {:else if id === "storage"}
          <StoragePanel
            stats={systemStats}
            {capturedAt}
            {history}
            {isFrozen}
            {unavailable}
            diskTotalBytes={systemStats.disk_total_bytes}
            diskUsedBytes={systemStats.disk_used_bytes}
            diskFreeBytes={systemStats.disk_free_bytes}
            onSpace={onOpenSpace}
            onDiskHealth={onOpenDiskHealth}
          />
        {:else if id === "system"}
          <SystemPanel
            stats={systemStats}
            {capturedAt}
            {isFrozen}
            {unavailable}
            {started}
          />
        {:else if id === "network"}
          <NetworkPanel
            stats={systemStats}
            {capturedAt}
            {history}
            {isFrozen}
            {unavailable}
            networkRxBytes={systemStats.network_rx_bytes}
            networkTxBytes={systemStats.network_tx_bytes}
          />
        {:else if id === "temp"}
          <TempPanel
            stats={systemStats}
            {capturedAt}
            {sensors}
            {sources}
            {history}
            {isFrozen}
            {unavailable}
          />
        {/if}
      {/each}
    </section></Tooltip.Provider
  >{/if}
