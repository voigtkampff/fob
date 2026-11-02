<script lang="ts">
  import type { Snippet } from "svelte";
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { ArrowUpRight, Pause, Play } from "@lucide/svelte";
  import { processStore } from "$lib/stores/processes";
  import type { SystemStats, ResourceSample } from "$lib/types";
  import ResourceDetailsContent from "./ResourceDetailsContent.svelte";
  import { t } from "$lib/i18n";
  import { clockTime } from "$lib/utils/chart-cursor.js";
  const panels = {
    cpu: 0,
    memory: 0,
    system: 0,
    network: 0,
    gpu: 0,
  } as const;
  let {
    kind,
    stats,
    history = [],
    isFrozen = false,
    unavailable = false,
    capturedAt = 0,
    started = null,
    children,
  }: {
    kind: keyof typeof panels;
    stats?: SystemStats;
    history?: ResourceSample[];
    isFrozen?: boolean;
    unavailable?: boolean;
    capturedAt?: number;
    started?: [number, string] | null;
    children?: Snippet;
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
    gpu: {
      title: $t("rd.gpu.title"),
      label: $t("rd.gpu.label"),
      description: $t("rd.gpu.description"),
    },
  });
  const panel = $derived(texts[kind]);
  const headingId = $derived(`resource-details-${kind}`);
  const triggerId = $derived(`resource-trigger-${kind}`);
</script>

<Sheet.Root onOpenChange={(open) => {}}>
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
    class="data-[side=right]:sm:max-w-md"
    onCloseAutoFocus={(event) => {
      event.preventDefault();
      document.getElementById(triggerId)?.focus();
    }}
    onOpenAutoFocus={(event) => {
      event.preventDefault();
      document.getElementById(headingId)?.focus();
    }}
  >
    <Sheet.Header class="border-b border-border pb-4 pr-12">
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
    <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-5 space-y-5">
      {#if children}{@render children()}{:else if stats && kind !== "gpu"}<ResourceDetailsContent
          {kind}
          {stats}
          {history}
          {isFrozen}
          {unavailable}
          {started}
        />{/if}
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
