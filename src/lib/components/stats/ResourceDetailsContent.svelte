<script lang="ts">
  import { onMount } from "svelte";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as NativeSelect from "$lib/components/ui/native-select/index.js";
  import { Progress } from "$lib/components/ui/progress/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { ArrowDown, ArrowUp, Network } from "@lucide/svelte";
  import { formatBytes } from "$lib/utils";
  import { formatDuration, uptimeInfo } from "$lib/utils/dashboard.js";
  import CoreGrid from "./CoreGrid.svelte";
  import { t } from "$lib/i18n";
  import type { SystemStats, ResourceSample } from "$lib/types";
  import ResourceTrend from "./ResourceTrend.svelte";
  let {
    kind,
    stats,
    history,
    isFrozen,
    unavailable,
    started = null,
  }: {
    kind: "cpu" | "memory" | "system" | "network";
    stats: SystemStats;
    history: ResourceSample[];
    isFrozen: boolean;
    unavailable: boolean;
    /** [Unix seconds, kind] of the last power-on (Extra.started). */
    started?: [number, string] | null;
  } = $props();
  const since = $derived(
    uptimeInfo(stats.uptime, started?.[0], Date.now() / 1000),
  );
  let interfaceSearch = $state("");
  let includeUnused = $state(false);
  let interfaceOrder = $state<string[]>([]);
  // Prioritize observed traffic on open, then keep rows stable while reading.
  onMount(() => {
    interfaceOrder = [...stats.details.interfaces]
      .sort(
        (a, b) =>
          (b.received ?? 0) +
            (b.sent ?? 0) -
            ((a.received ?? 0) + (a.sent ?? 0)) ||
          b.received_total + b.sent_total - (a.received_total + a.sent_total) ||
          a.name.localeCompare(b.name),
      )
      .map((i) => i.name);
  });
  const details = $derived(stats.details);
  const identity = $derived(details.identity);
  const cpu = $derived(
    stats.cpu_usage.length
      ? stats.cpu_usage.reduce((a, b) => a + b, 0) / stats.cpu_usage.length
      : null,
  );
  const memory = $derived(
    stats.memory_total > 0
      ? Math.min(100, (stats.memory_used / stats.memory_total) * 100)
      : null,
  );
  const interfaceRanks = $derived(
    new Map(interfaceOrder.map((name, index) => [name, index])),
  );
  const interfaces = $derived(
    details.interfaces
      .filter(
        (i) =>
          (includeUnused ||
            interfaceSearch.trim() ||
            i.received_total > 0 ||
            i.sent_total > 0 ||
            i.received === null ||
            i.sent === null) &&
          i.name.toLowerCase().includes(interfaceSearch.trim().toLowerCase()),
      )
      .sort(
        (a, b) =>
          (interfaceRanks.get(a.name) ?? Infinity) -
            (interfaceRanks.get(b.name) ?? Infinity) ||
          a.name.localeCompare(b.name),
      ),
  );
  const percent = (value: number | null) =>
    value === null || !Number.isFinite(value)
      ? $t("common.na")
      : `${value.toFixed(1)}%`;
  const rate = (value: number | null) =>
    value === null ? $t("common.na") : `${formatBytes(value)}/s`;
</script>

{#snippet facts(items: [string, string][])}
  <Card.Root size="sm"
    ><Card.Content class="gap-0 divide-y divide-border">
      {#each items as [label, value]}<div
          class="flex justify-between gap-4 py-3 first:pt-0 last:pb-0"
        >
          <span class="text-sm text-muted-foreground">{label}</span><span
            class="min-w-0 max-w-[65%] break-words text-right text-sm font-medium tabular-nums"
            data-selectable>{value}</span
          >
        </div>{/each}
    </Card.Content></Card.Root
  >
{/snippet}
{#snippet note(text: string)}<p
    class="rounded-lg border border-border bg-muted/40 p-4 text-sm leading-relaxed text-muted-foreground"
  >
    {text}
  </p>{/snippet}

{#if kind === "cpu"}
  <div>
    <h3 class="text-lg font-semibold">
      {identity.cpu_model ?? $t("details.processor")}
    </h3>
    <p class="mt-1 text-sm text-muted-foreground">
      {$t("details.logicalArch", {
        n: stats.cpu_usage.length,
        arch: identity.architecture,
      })}
    </p>
  </div>
  {@render facts([
    [$t("stats.avgUtil"), percent(cpu)],
    [
      $t("details.physicalCores"),
      identity.physical_cores === null
        ? $t("common.na")
        : String(identity.physical_cores),
    ],
    [$t("details.logical"), String(stats.cpu_usage.length)],
  ])}
  <section class="space-y-2" aria-label={$t("details.cpuHistory")}>
    <h3 class="text-sm font-medium">{$t("details.overTime")}</h3>
    <ResourceTrend
      {history}
      metric="cpu"
      label={$t("stats.cpuUtil")}
      expanded
      {isFrozen}
      {unavailable}
    />
  </section>
  <Card.Root size="sm"
    ><Card.Header
      ><Card.Title class="text-sm">{$t("details.perCore")}</Card.Title
      ><Card.Description>{$t("details.perCoreNote")}</Card.Description
      ></Card.Header
    ><Card.Content
      ><CoreGrid usage={stats.cpu_usage} where="detail" /></Card.Content
    ></Card.Root
  >
  {@render note($t("details.cpuNote"))}
{:else if kind === "memory"}
  <div>
    <p class="text-3xl font-semibold tracking-tight tabular-nums">
      {formatBytes(stats.memory_used)}
    </p>
    <p class="mt-1 text-sm text-muted-foreground">
      {$t("details.ramInUse", {
        p: percent(memory),
        v: formatBytes(stats.memory_total),
      })}
    </p>
  </div>
  <ResourceTrend
    {history}
    metric="memory"
    label={$t("stats.memUtil")}
    expanded
    {isFrozen}
    {unavailable}
  />
  {@render facts([
    [$t("details.physicalMemory"), formatBytes(stats.memory_total)],
    [$t("details.inUse"), formatBytes(stats.memory_used)],
    [$t("stats.notInUse"), formatBytes(stats.memory_free)],
    [$t("details.unallocated"), formatBytes(details.memory_unallocated)],
  ])}
  <Card.Root size="sm"
    ><Card.Header
      ><Card.Title class="text-sm">{$t("details.swap")}</Card.Title
      ><Card.Description>{$t("details.swapNote")}</Card.Description
      ></Card.Header
    ><Card.Content>
      {#if details.swap_total > 0}<div
          class="flex justify-between gap-3 text-sm"
        >
          <span class="tabular-nums"
            >{$t("dash.used", { v: formatBytes(details.swap_used) })}</span
          ><span class="text-muted-foreground"
            >{$t("details.ofTotal", {
              v: formatBytes(details.swap_total),
            })}</span
          >
        </div>
        <Progress
          value={Math.min(100, (details.swap_used / details.swap_total) * 100)}
          aria-label={$t("details.swapUsed")}
          style="--primary:var(--chart-3)"
        />{:else}<p class="text-sm text-muted-foreground">
          {$t("details.noSwap")}
        </p>{/if}
    </Card.Content></Card.Root
  >
  {@render note($t("details.memNote"))}
{:else if kind === "system"}
  <div>
    <h3 class="break-words text-lg font-semibold">
      {identity.os_version ?? $t("details.noOs")}
    </h3>
    <p class="mt-1 text-sm text-muted-foreground">{identity.architecture}</p>
  </div>
  {@render facts([
    [
      started ? $t("details.sincePowerOn") : $t("stats.kernelUptime"),
      formatDuration(since.seconds, $t),
    ],
    ...(started
      ? [
          [$t("stats.kernel"), formatDuration(since.kernel, $t)] as [
            string,
            string,
          ],
        ]
      : []),
    [$t("details.kernelVersion"), identity.kernel_version ?? $t("common.na")],
    [$t("details.architecture"), identity.architecture],
    [$t("details.processor"), identity.cpu_model ?? $t("common.na")],
    [$t("details.logical"), String(stats.cpu_usage.length)],
  ])}
  {@render note($t("details.systemNote"))}
{:else if kind === "network"}
  <Card.Root size="sm"
    ><Card.Content class="grid grid-cols-2 gap-4"
      >{#each [{ label: $t("stats.received"), value: stats.network_rx_bytes, icon: ArrowDown, color: "var(--chart-1)" }, { label: $t("stats.sent"), value: stats.network_tx_bytes, icon: ArrowUp, color: "var(--chart-4)" }] as metric}<div
          class="min-w-0"
        >
          <p class="flex items-center gap-1.5 text-xs text-muted-foreground">
            <metric.icon
              class="size-3"
              style={`color:${metric.color}`}
            />{metric.label}
          </p>
          <p class="mt-1 break-words text-xl font-semibold tabular-nums">
            {rate(metric.value)}
          </p>
        </div>{/each}</Card.Content
    ></Card.Root
  >
  <ResourceTrend
    {history}
    metric="network"
    label={$t("stats.netTrend")}
    expanded
    {isFrozen}
    {unavailable}
  />
  <section class="space-y-3" aria-label={$t("details.interfacesAria")}>
    <div class="flex items-center justify-between gap-2">
      <h3 class="text-sm font-medium">{$t("details.interfaces")}</h3>
      <span class="text-xs text-muted-foreground"
        >{$t("details.nOfM", {
          n: interfaces.length,
          m: details.interfaces.length,
        })}</span
      >
    </div>
    <Input
      type="search"
      placeholder={$t("details.searchInterfaces")}
      aria-label={$t("details.findInterface")}
      bind:value={interfaceSearch}
    />
    <div class="flex items-center gap-2">
      <Checkbox
        id="include-unused-interfaces"
        checked={includeUnused}
        onCheckedChange={(value) => (includeUnused = value === true)}
      /><Label
        for="include-unused-interfaces"
        class="text-xs font-normal text-muted-foreground"
        >{$t("details.includeUnused")}</Label
      >
    </div>
    {#each interfaces as item (item.name)}<Card.Root size="sm" class="gap-3"
        ><Card.Header
          ><Card.Title class="break-all font-mono text-sm"
            >{item.name}</Card.Title
          ></Card.Header
        ><Card.Content
          ><div class="grid grid-cols-2 gap-4">
            <div>
              <p class="text-xs text-muted-foreground">
                ↓ {$t("stats.received")} /s
              </p>
              <p class="mt-1 text-sm font-medium tabular-nums">
                {rate(item.received)}
              </p>
            </div>
            <div>
              <p class="text-xs text-muted-foreground">
                ↑ {$t("stats.sent")} /s
              </p>
              <p class="mt-1 text-sm font-medium tabular-nums">
                {rate(item.sent)}
              </p>
            </div>
          </div>
          <dl
            class="grid grid-cols-2 gap-4 border-t border-border pt-2 text-xs"
          >
            <div>
              <dt class="text-muted-foreground">
                {$t("details.receivedTotal")}
              </dt>
              <dd class="mt-1 tabular-nums">
                {formatBytes(item.received_total)}
              </dd>
            </div>
            <div>
              <dt class="text-muted-foreground">{$t("details.sentTotal")}</dt>
              <dd class="mt-1 tabular-nums">{formatBytes(item.sent_total)}</dd>
            </div>
          </dl></Card.Content
        ></Card.Root
      >{:else}<div
        class="rounded-lg border border-dashed border-border p-5 text-sm text-muted-foreground"
      >
        <Network class="mb-2 size-5" />
        <p>
          {interfaceSearch.trim()
            ? $t("details.noInterfaceMatch")
            : details.interfaces.length
              ? $t("details.noTraffic")
              : $t("details.noInterfaces")}
        </p>
        {#if interfaceSearch}<Button
            variant="link"
            size="sm"
            class="mt-2 px-0"
            onclick={() => (interfaceSearch = "")}>{$t("search.clear")}</Button
          >{/if}
        {#if !interfaceSearch.trim() && details.interfaces.length && !includeUnused}<Button
            variant="link"
            size="sm"
            class="mt-2 px-0"
            onclick={() => (includeUnused = true)}
            >{$t("details.showAllInterfaces")}</Button
          >{/if}
      </div>{/each}
  </section>
  {@render note($t("details.netNote"))}
{/if}
