<script lang="ts">
  // Fob: the side panel shell every stats-row card opens: header (title,
  // subtitle, reading time), the scrolling body, footer (pause / done).
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { ArrowUpRight, Pause, Play } from "@lucide/svelte";
  import { processStore } from "$lib/stores/processes";
  import type {
    SystemStats,
    ResourceSample,
    GpuDevice,
    Sensor,
  } from "$lib/types";
  import ResourceDetailsContent from "./ResourceDetailsContent.svelte";
  import type { PanelKind } from "./panel";
  import { t } from "$lib/i18n";
  import { clockTime } from "$lib/utils/chart-cursor.js";
  let {
    kind,
    stats,
    history = [],
    isFrozen = false,
    unavailable = false,
    capturedAt = 0,
    started = null,
    gpus = [],
    gpuId = "",
    onGpu = () => {},
    sensors = [],
    sources = [],
    onSpace = () => {},
    onDiskHealth = () => {},
    open = $bindable(false),
  }: {
    kind: PanelKind;
    stats: SystemStats;
    history?: ResourceSample[];
    isFrozen?: boolean;
    unavailable?: boolean;
    capturedAt?: number;
    started?: [number, string] | null;
    gpus?: GpuDevice[];
    gpuId?: string;
    onGpu?: (id: string) => void;
    sensors?: Sensor[];
    sources?: string[];
    onSpace?: () => void;
    onDiskHealth?: () => void;
    open?: boolean;
  } = $props();
  // Literal keys so the i18n check can see every one.
  const texts = $derived({
    cpu: {
      title: $t("rd.cpu.title"),
      label: $t("rd.cpu.label"),
      description: $t("rd.cpu.description"),
    },
    memory: {
      title: $t("rd.memory.title"),
      label: $t("rd.memory.label"),
      description: $t("rd.memory.description"),
    },
    gpu: {
      title: $t("rd.gpu.title"),
      label: $t("rd.gpu.label"),
      description: $t("rd.gpu.description"),
    },
    storage: {
      title: $t("rd.storage.title"),
      label: $t("rd.storage.label"),
      description: $t("rd.storage.description"),
    },
    system: {
      title: $t("rd.system.title"),
      label: $t("rd.system.label"),
      description: $t("rd.system.description"),
    },
    network: {
      title: $t("rd.network.title"),
      label: $t("rd.network.label"),
      description: $t("rd.network.description"),
    },
    temp: {
      title: $t("rd.temp.title"),
      label: $t("rd.temp.label"),
      description: $t("rd.temp.description"),
    },
  });
  const panel = $derived(texts[kind]);
  const headingId = $derived(`resource-details-${kind}`);
  const triggerId = $derived(`resource-trigger-${kind}`);
  // A link that leaves the panel runs after the panel has closed, so focus
  // and the page's pointer lock are released before the page changes.
  let afterClose: (() => void) | null = null;
</script>

<Sheet.Root bind:open>
  <Sheet.Trigger id={triggerId}
    >{#snippet child({ props })}<Button
        {...props}
        variant="ghost"
        size="icon-xs"
        aria-label={panel.label}
        title={panel.label}><ArrowUpRight class="size-3.5" /></Button
      >{/snippet}</Sheet.Trigger
  >
  <Sheet.Content
    class="gap-0 data-[side=right]:sm:max-w-md"
    onCloseAutoFocus={(event) => {
      event.preventDefault();
      const next = afterClose;
      afterClose = null;
      if (next) next();
      else document.getElementById(triggerId)?.focus();
    }}
    onOpenAutoFocus={(event) => {
      event.preventDefault();
      document.getElementById(headingId)?.focus();
    }}
  >
    <Sheet.Header class="shrink-0 border-b border-border pb-4 pr-12">
      <Sheet.Title id={headingId} tabindex={-1} class="outline-none"
        >{panel.title}</Sheet.Title
      >
      <Sheet.Description>{panel.description}</Sheet.Description>
      <div class="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
        {#if unavailable}<Badge variant="destructive"
            >{$t("trend.unavailable")}</Badge
          >{:else if isFrozen}<Badge variant="secondary"
            >{$t("mini.paused")}</Badge
          >{:else}<span
            class="size-1.5 rounded-full bg-[var(--status-running)]"
            aria-hidden="true"
          ></span>{/if}
        <span
          >{capturedAt
            ? $t("rd.readingFrom", { v: clockTime(capturedAt) })
            : $t("rd.waiting")}</span
        >
      </div>
    </Sheet.Header>
    <!-- The body is the scroll container and clips at its padding box. Cards
         draw their border as a ring (a box-shadow outside the box), so the
         body keeps padding on every side or a first card loses its top edge. -->
    <div class="panel-body min-h-0 flex-1 overflow-y-auto px-4 pt-5 pb-5">
      <ResourceDetailsContent
        {kind}
        {stats}
        {history}
        {isFrozen}
        {unavailable}
        {started}
        {gpus}
        {gpuId}
        {onGpu}
        {sensors}
        {sources}
        onSpace={() => {
          open = false;
          onSpace();
        }}
        onDiskHealth={() => {
          afterClose = onDiskHealth;
          open = false;
        }}
      />
    </div>
    <Sheet.Footer
      class="shrink-0 flex-row items-center justify-between border-t border-border px-4 py-3"
    >
      <Button
        variant="outline"
        size="sm"
        aria-label={isFrozen
          ? $t("rd.resumeMonitoring")
          : $t("rd.pauseMonitoring")}
        onclick={() => processStore.setIsFrozen(!isFrozen)}
        >{#if isFrozen}<Play />{$t("rd.resume")}{:else}<Pause />{$t(
            "rd.pause",
          )}{/if}</Button
      >
      <Sheet.Close
        >{#snippet child({ props })}<Button
            {...props}
            variant="secondary"
            size="sm">{$t("rd.done")}</Button
          >{/snippet}</Sheet.Close
      >
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
