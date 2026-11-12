<script lang="ts">
  // Fob: what each side panel shows, all laid out by DetailLayout (same
  // order, spacing and card style for every card of the stats row).
  import { onMount } from "svelte";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as NativeSelect from "$lib/components/ui/native-select/index.js";
  import { Progress } from "$lib/components/ui/progress/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { LayoutGrid, Network } from "@lucide/svelte";
  import { formatBytes } from "$lib/utils";
  import {
    byKind,
    fanGroups,
    fanText,
    formatDuration,
    formatSensor,
    groupByHardware,
    sensorSummary,
    setupState,
    uptimeInfo,
  } from "$lib/utils/dashboard.js";
  import { sensorSetup } from "$lib/stores/sensorSetup";
  import SensorSetup from "$lib/components/dashboard/SensorSetup.svelte";
  import CoreGrid from "./CoreGrid.svelte";
  import { cpuGroups } from "$lib/utils/cpu-groups.js";
  import DetailLayout from "./DetailLayout.svelte";
  import DetailSection from "./DetailSection.svelte";
  import ResourceTrend from "./ResourceTrend.svelte";
  import DiskHealthSummary from "./DiskHealthSummary.svelte";
  import { locale, t } from "$lib/i18n";
  import {
    engineLabel,
    engineTitle,
    gpuAboutNote,
  } from "$lib/utils/gpu-engines.js";
  import type { PanelKind } from "./panel";
  import type {
    GpuDevice,
    ResourceSample,
    Sensor,
    SystemStats,
  } from "$lib/types";
  let {
    kind,
    stats,
    history,
    isFrozen,
    unavailable,
    started = null,
    gpus = [],
    gpuId = "",
    onGpu = () => {},
    sensors = [],
    sources = [],
    onSpace = () => {},
    onDiskHealth = () => {},
  }: {
    kind: PanelKind;
    stats: SystemStats;
    history: ResourceSample[];
    isFrozen: boolean;
    unavailable: boolean;
    /** [Unix seconds, kind] of the last power-on (Extra.started). */
    started?: [number, string] | null;
    gpus?: GpuDevice[];
    gpuId?: string;
    onGpu?: (id: string) => void;
    sensors?: Sensor[];
    sources?: string[];
    onSpace?: () => void;
    /** Opens the full Disk detail (the storage panel's health link). */
    onDiskHealth?: () => void;
  } = $props();
  const details = $derived(stats.details);
  const identity = $derived(details.identity);
  const na = $derived($t("common.na"));
  const percent = (value: number | null | undefined) =>
    value == null || !Number.isFinite(value) ? na : `${value.toFixed(1)}%`;
  const rate = (value: number | null | undefined) =>
    value == null ? na : `${formatBytes(value)}/s`;
  const bytes = (value: number | null | undefined) =>
    value == null ? na : formatBytes(value);
  const share = (part: number, total: number) =>
    total > 0 ? Math.min(100, Math.max(0, (part / total) * 100)) : 0;

  // CPU
  const cpu = $derived(
    stats.cpu_usage.length
      ? stats.cpu_usage.reduce((a, b) => a + b, 0) / stats.cpu_usage.length
      : null,
  );
  const coreGroups = $derived(cpuGroups(stats.cpu_topology, stats.cpu_usage));
  // Memory
  const memory = $derived(
    stats.memory_total > 0
      ? Math.min(100, (stats.memory_used / stats.memory_total) * 100)
      : null,
  );
  // System
  const since = $derived(
    uptimeInfo(stats.uptime, started?.[0], Date.now() / 1000),
  );
  // GPU
  const device = $derived(
    gpus.find((g) => g.id === gpuId) ??
      gpus.find((g) => g.utilization !== null) ??
      gpus[0],
  );
  // Storage
  const volumes = $derived(
    // Pseudo mounts with no capacity (Linux) aren't drives.
    details.volumes
      .filter((v) => v.total > 0)
      .sort((a, b) => a.mount.localeCompare(b.mount)),
  );
  const storageUsed = $derived(
    share(stats.disk_used_bytes, stats.disk_total_bytes),
  );
  // Temp & fan
  const summary = $derived(sensorSummary(sensors));
  const fans = $derived(fanGroups(sensors));
  const sensorGroups = $derived(
    groupByHardware(sensors.filter((s) => s.kind !== "fan")),
  );
  const fanCount = $derived(fans.reduce((n, g) => n + g.fans.length, 0));
  const setup = $derived(setupState($sensorSetup));
  const head = $derived(summary.cpuTemp ?? summary.gpuTemp);
  // Literal keys so the i18n check can see every one.
  const helperText = $derived(
    {
      none: $t("rd.temp.helperNone"),
      ready: $t("rd.temp.helperReady"),
      starting: $t("rd.temp.helperStarting"),
      enable: $t("rd.temp.helperOff"),
      repair: $t("rd.temp.helperRepair"),
      update: $t("rd.temp.helperUpdate"),
      "missing-files": $t("rd.temp.helperMissing"),
    }[setup] ?? setup,
  );
  // Network: busiest first on open, then rows stay put while reading.
  let interfaceSearch = $state("");
  let includeUnused = $state(false);
  let interfaceOrder = $state<string[]>([]);
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
  const interfaceRanks = $derived(
    new Map(interfaceOrder.map((name, index) => [name, index])),
  );
  const used = (i: SystemStats["details"]["interfaces"][number]) =>
    i.received_total > 0 ||
    i.sent_total > 0 ||
    i.received === null ||
    i.sent === null;
  const interfaces = $derived(
    details.interfaces
      .filter(
        (i) =>
          (includeUnused || interfaceSearch.trim() || used(i)) &&
          i.name.toLowerCase().includes(interfaceSearch.trim().toLowerCase()),
      )
      .sort(
        (a, b) =>
          (interfaceRanks.get(a.name) ?? Infinity) -
            (interfaceRanks.get(b.name) ?? Infinity) ||
          a.name.localeCompare(b.name),
      ),
  );
  // The identity line names the busiest real adapter (not the loopback).
  const busiest = $derived(
    [...details.interfaces]
      .filter((i) => !/^lo$|loopback/i.test(i.name))
      .sort(
        (a, b) =>
          (b.received ?? 0) +
          (b.sent ?? 0) -
          ((a.received ?? 0) + (a.sent ?? 0)),
      )[0],
  );
  const sum = (pick: (i: (typeof details.interfaces)[number]) => number) =>
    details.interfaces.reduce((n, i) => n + pick(i), 0);
</script>

{#snippet trend(
  metric: "cpu" | "memory" | "network" | "gpu" | "temp" | "disk",
  label: string,
  unsupported = false,
)}<ResourceTrend
    {history}
    {metric}
    {label}
    gpuId={device?.id ?? ""}
    expanded
    {isFrozen}
    {unavailable}
    {unsupported}
  />{/snippet}
{#snippet row(label: string, value: string, muted = false)}<div
    class="flex justify-between gap-4 text-sm"
  >
    <span class="min-w-0 truncate text-muted-foreground" title={label}
      >{label}</span
    ><span
      class={`shrink-0 text-right font-medium tabular-nums ${muted ? "text-muted-foreground" : ""}`}
      >{value}</span
    >
  </div>{/snippet}

{#if kind === "cpu"}
  <DetailLayout
    name={identity.cpu_model ?? $t("details.processor")}
    sub={$t("details.logicalArch", {
      n: stats.cpu_usage.length,
      arch: identity.architecture,
    })}
    facts={[
      [$t("stats.avgUtil"), percent(cpu)],
      ...(coreGroups ?? []).map((g): [string, string] => [
        $t(g.label),
        percent(g.usage),
      ]),
      [
        $t("details.physicalCores"),
        identity.physical_cores === null ? na : String(identity.physical_cores),
      ],
      [$t("details.logical"), String(stats.cpu_usage.length)],
    ]}
    chartTitle={$t("details.overTime")}
    note={$t("details.cpuNote")}
  >
    {#snippet chart()}{@render trend("cpu", $t("stats.cpuUtil"))}{/snippet}
    {#snippet detail()}<DetailSection
        title={$t("details.perCore")}
        description={$t(coreGroups ? "cpu.groupsNote" : "details.perCoreNote")}
        ><CoreGrid
          usage={stats.cpu_usage}
          where="detail"
          groups={coreGroups}
        /></DetailSection
      >{/snippet}
  </DetailLayout>
{:else if kind === "memory"}
  <DetailLayout
    name={$t("rd.memory.identity", { v: formatBytes(stats.memory_total) })}
    sub={$t("details.ramInUse", {
      p: percent(memory),
      v: formatBytes(stats.memory_total),
    })}
    facts={[
      [$t("details.inUse"), formatBytes(stats.memory_used)],
      [$t("stats.notInUse"), formatBytes(stats.memory_free)],
      [$t("details.unallocated"), formatBytes(details.memory_unallocated)],
      [$t("details.physicalMemory"), formatBytes(stats.memory_total)],
    ]}
    chartTitle={$t("details.overTime")}
    note={$t("details.memNote")}
  >
    {#snippet chart()}{@render trend("memory", $t("stats.memUtil"))}{/snippet}
    {#snippet detail()}<DetailSection
        title={$t("details.swap")}
        description={$t("details.swapNote")}
      >
        {#if details.swap_total > 0}{@render row(
            $t("dash.used", { v: formatBytes(details.swap_used) }),
            $t("details.ofTotal", { v: formatBytes(details.swap_total) }),
          )}
          <Progress
            value={share(details.swap_used, details.swap_total)}
            aria-label={$t("details.swapUsed")}
            style="--primary:var(--chart-3)"
          />{:else}<p class="text-sm text-muted-foreground">
            {$t("details.noSwap")}
          </p>{/if}
      </DetailSection>{/snippet}
  </DetailLayout>
{:else if kind === "gpu"}
  {#if device}
    <DetailLayout
      name={device.name}
      sub={`${device.vendor}${device.cores ? ` · ${$t("gpu.cores", { n: device.cores })}` : ""}`}
      facts={[
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
      ]}
      chartTitle={$t("details.overTime")}
      noteTitle={$t("gpu.about")}
      note={gpuAboutNote(device.source, device.memory_kind, $t)}
    >
      {#snippet picker()}{#if gpus.length > 1}<div class="space-y-2">
            <label for="gpu-device" class="text-sm font-medium"
              >{$t("gpu.adapter")}</label
            ><NativeSelect.Root
              id="gpu-device"
              value={device.id}
              onchange={(event) => onGpu(event.currentTarget.value)}
              class="w-full"
              >{#each gpus as gpu}<option value={gpu.id}>{gpu.name}</option
                >{/each}</NativeSelect.Root
            >
          </div>{/if}{/snippet}
      {#snippet chart()}{@render trend(
          "gpu",
          $t("stats.gpuUtilOf", { name: device.name }),
          device.utilization === null,
        )}{/snippet}
      {#snippet detail()}<DetailSection title={$t("rd.gpu.engines")}>
          {#if device.memory_used != null && device.memory_total}{@render row(
              device.memory_kind === "shared"
                ? $t("gpu.sharedInUse")
                : $t("gpu.memInUse"),
              `${formatBytes(device.memory_used)} / ${formatBytes(device.memory_total)}`,
            )}<Progress
              value={share(device.memory_used, device.memory_total)}
              aria-label={$t("gpu.memInUse")}
              style="--primary:var(--chart-2)"
            />{/if}
          {#each device.engines ?? [] as [engine, load]}<div
              class="grid grid-cols-[7rem_1fr_3rem] items-center gap-3 text-sm"
            >
              <span
                class="truncate text-muted-foreground"
                data-engine={engine}
                title={engineTitle(engine, $t)}
                >{engineLabel(engine, $locale)}</span
              ><Progress
                value={Math.min(100, Math.max(0, load))}
                aria-label={engineLabel(engine, $locale)}
                style="--primary:var(--chart-2)"
              /><span class="text-right font-medium tabular-nums"
                >{load.toFixed(0)}%</span
              >
            </div>{:else}<p class="text-sm text-muted-foreground">
              {$t("rd.gpu.noEngines")}
            </p>{/each}
        </DetailSection>{/snippet}
    </DetailLayout>
  {:else}
    <DetailLayout
      name={$t("gpu.unavailable")}
      note={$t("gpu.unavailableNote")}
    />
  {/if}
{:else if kind === "storage"}
  <DetailLayout
    name={$t("rd.storage.identity", { n: volumes.length })}
    sub={stats.disk_total_bytes > 0
      ? $t("rd.storage.sub", {
          p: storageUsed.toFixed(1),
          v: formatBytes(stats.disk_total_bytes),
        })
      : $t("stats.unknownCapacity")}
    facts={[
      [$t("rd.storage.free"), formatBytes(stats.disk_free_bytes)],
      [$t("stats.used"), formatBytes(stats.disk_used_bytes)],
      [$t("rd.storage.total"), formatBytes(stats.disk_total_bytes)],
      [$t("rd.storage.read"), rate(stats.disk_read_bytes)],
      [$t("rd.storage.write"), rate(stats.disk_write_bytes)],
    ]}
    chartTitle={$t("rd.storage.overTime")}
    note={$t("rd.storage.note")}
  >
    {#snippet chart()}{@render trend(
        "disk",
        $t("rd.storage.overTime"),
        stats.disk_read_bytes == null,
      )}{/snippet}
    {#snippet detail()}<DetailSection title={$t("detail.disk")}>
        {#snippet action()}<Button
            variant="outline"
            size="sm"
            class="h-7 shrink-0 text-xs"
            onclick={onSpace}><LayoutGrid />{$t("space.title")}</Button
          >{/snippet}
        {#each volumes as v (v.mount)}<div
            class="space-y-1.5 border-b border-border pb-3 last:border-0 last:pb-0"
          >
            {@render row(
              v.mount,
              $t("detail.freeOf", {
                free: formatBytes(v.available),
                total: formatBytes(v.total),
              }),
            )}
            <Progress
              value={share(v.total - v.available, v.total)}
              aria-label={$t("rd.storage.usedOf", { v: v.mount })}
              style="--primary:var(--chart-4)"
            />
            <p
              class="flex justify-between gap-3 text-xs text-muted-foreground tabular-nums"
            >
              <span class="min-w-0 truncate"
                >{v.name || v.file_system}{v.file_system && v.name
                  ? ` · ${v.file_system}`
                  : ""}{v.read_only ? ` · ${$t("detail.readOnly")}` : ""}</span
              ><span class="shrink-0"
                >{$t("stats.read")}
                {rate(v.read)} · {$t("stats.write")}
                {rate(v.written)}</span
              >
            </p>
          </div>{:else}<p class="text-sm text-muted-foreground">
            {$t("detail.noDrives")}
          </p>{/each}
      </DetailSection>
      <DiskHealthSummary onDetails={onDiskHealth} />{/snippet}
  </DetailLayout>
{:else if kind === "system"}
  <DetailLayout
    name={identity.os_version ?? $t("details.noOs")}
    sub={identity.architecture}
    facts={[
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
      [$t("details.kernelVersion"), identity.kernel_version ?? na],
    ]}
    note={$t("details.systemNote")}
  >
    {#snippet detail()}<DetailSection title={$t("rd.system.platform")}>
        {@render row($t("details.processor"), identity.cpu_model ?? na)}
        {@render row(
          $t("details.physicalCores"),
          identity.physical_cores === null
            ? na
            : String(identity.physical_cores),
        )}
        {@render row($t("details.logical"), String(stats.cpu_usage.length))}
        {@render row($t("details.architecture"), identity.architecture)}
        {@render row(
          $t("details.physicalMemory"),
          formatBytes(stats.memory_total),
        )}
      </DetailSection>{/snippet}
  </DetailLayout>
{:else if kind === "network"}
  <DetailLayout
    name={busiest?.name ?? $t("rd.network.none")}
    sub={$t("rd.network.sub", {
      n: details.interfaces.filter(used).length,
      m: details.interfaces.length,
    })}
    facts={[
      [`↓ ${$t("stats.received")}`, rate(stats.network_rx_bytes)],
      [`↑ ${$t("stats.sent")}`, rate(stats.network_tx_bytes)],
      [$t("details.receivedTotal"), formatBytes(sum((i) => i.received_total))],
      [$t("details.sentTotal"), formatBytes(sum((i) => i.sent_total))],
    ]}
    chartTitle={$t("rd.network.overTime")}
    note={$t("details.netNote")}
  >
    {#snippet chart()}{@render trend("network", $t("stats.netTrend"))}{/snippet}
    {#snippet detail()}<DetailSection
        title={$t("details.interfaces")}
        aside={$t("details.nOfM", {
          n: interfaces.length,
          m: details.interfaces.length,
        })}
      >
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
        {#each interfaces as item (item.name)}<div
            class="space-y-1.5 border-t border-border pt-3"
          >
            <p class="break-all font-mono text-sm font-medium">{item.name}</p>
            {@render row(
              `↓ ${$t("stats.received")} · ↑ ${$t("stats.sent")}`,
              `${rate(item.received)} · ${rate(item.sent)}`,
            )}
            {@render row(
              $t("rd.network.totals"),
              `${formatBytes(item.received_total)} · ${formatBytes(item.sent_total)}`,
              true,
            )}
          </div>{:else}<div
            class="border-t border-border pt-3 text-sm text-muted-foreground"
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
                onclick={() => (interfaceSearch = "")}
                >{$t("search.clear")}</Button
              >{/if}
            {#if !interfaceSearch.trim() && details.interfaces.length && !includeUnused}<Button
                variant="link"
                size="sm"
                class="mt-2 px-0"
                onclick={() => (includeUnused = true)}
                >{$t("details.showAllInterfaces")}</Button
              >{/if}
          </div>{/each}
      </DetailSection>{/snippet}
  </DetailLayout>
{:else if kind === "temp"}
  <DetailLayout
    name={head ? head.name : $t("rd.temp.none")}
    sub={$t("rd.temp.sub", {
      n: sensors.filter((s) => s.kind === "temperature").length,
      f: fanCount,
    })}
    facts={[
      [$t("rd.temp.cpu"), summary.cpuTemp ? formatSensor(summary.cpuTemp) : na],
      [$t("rd.temp.gpu"), summary.gpuTemp ? formatSensor(summary.gpuTemp) : na],
      [
        $t("stats.fans"),
        fanText(sensors) || (summary.fan ? formatSensor(summary.fan) : na),
      ],
      [$t("rd.temp.helper"), helperText],
      ...($sensorSetup?.supported
        ? [
            [
              "PawnIO",
              $sensorSetup.pawnio_installed
                ? $t("rd.temp.installed")
                : $t("rd.temp.notInstalled"),
            ] as [string, string],
          ]
        : []),
    ]}
    chartTitle={$t("rd.temp.overTime")}
    note={`${$t("rd.temp.note")}${sources.length ? ` ${$t("detail.source", { v: sources.join(", ") })}` : ""}`}
  >
    {#snippet chart()}{@render trend("temp", $t("dash.temp"), !head)}{/snippet}
    {#snippet detail()}
      {#if setup !== "none"}<DetailSection title={$t("rd.temp.helper")}
          ><SensorSetup status={$sensorSetup} /></DetailSection
        >{/if}
      {#each fans as group (group.hardware)}<DetailSection
          title={$t("detail.fansOf", { hw: group.hardware })}
          aside={$t("detail.rpm")}
        >
          {#each group.fans as f (f.id || f.name)}{@render row(
              f.name,
              f.value > 0
                ? String(Math.round(f.value))
                : $t("detail.fanStopped"),
              f.value <= 0,
            )}{/each}
        </DetailSection>{/each}
      {#each sensorGroups as group (group.hardware)}<DetailSection
          title={group.hardware}
        >
          {#each byKind(group.sensors) as s (s.id || `${s.kind}:${s.name}`)}{@render row(
              `${s.name}${s.kind === "control" ? ` ${$t("detail.duty")}` : ""}`,
              formatSensor(s),
            )}{/each}
        </DetailSection>{/each}
      {#if !sensors.length}<DetailSection title={$t("rd.temp.sensors")}
          ><p class="text-sm text-muted-foreground">
            {setup !== "none"
              ? $t("detail.untilThen")
              : $t("detail.noReadings")}
          </p></DetailSection
        >{/if}
    {/snippet}
  </DetailLayout>
{/if}
