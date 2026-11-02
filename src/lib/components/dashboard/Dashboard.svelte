<script lang="ts">
  // winperf: card dashboard (default view). Reads only what NeoHtop's sampler
  // already provides: stats, GPU devices, the bounded history and processes.
  import CardDetail from "./CardDetail.svelte";
  import type { DetailKind } from "./detail";
  import type {
    FrameExtra,
    GpuDevice,
    Process,
    ResourceSample,
    SystemStats,
  } from "$lib/types";
  import {
    averageCpu,
    chartMax,
    formatGB,
    formatPercent,
    formatRate,
    formatDuration,
    uptimeInfo,
    gpuSummary,
    lastN,
    series,
    sparklinePath,
    systemVolume,
    topByMemory,
    topBy,
    sensorSummary,
    formatSensor,
    fanText,
    setupState,
  } from "$lib/utils/dashboard.js";
  import { onMount } from "svelte";
  import { sensorSetup, refreshSensorSetup } from "$lib/stores/sensorSetup";
  import {
    Cpu,
    MemoryStick,
    HardDrive,
    Globe,
    Gauge,
    Thermometer,
    Activity,
  } from "@lucide/svelte";

  export let systemStats: SystemStats;
  export let gpus: GpuDevice[] = [];
  export let history: ResourceSample[] = [];
  export let processes: (Pick<
    Process,
    "pid" | "name" | "cpu_usage" | "memory_usage"
  > & { command?: string; gpu_usage?: number | null })[] = [];
  /** winperf mini window (tray popup): tighter cards, no page title. */
  export let compact = false;
  export let extra: FrameExtra | null = null;
  /** Top lists from the backend (mini window); computed from `processes` otherwise. */
  export let topCpu: typeof processes | null = null;
  export let topGpu: typeof processes | null = null;
  export let topMemory: typeof processes | null = null;
  /** Cards open a detail view when clicked; `detail` is bindable (Esc handling). */
  export let expandable = false;
  export let detail: DetailKind | null = null;

  function open(kind: DetailKind) {
    if (expandable) detail = kind;
  }
  function openKey(event: KeyboardEvent, kind: DetailKind) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      open(kind);
    }
  }
  $: sensors = extra?.sensors.sensors ?? [];
  $: summary = sensorSummary(sensors);
  $: fansLine = fanText(sensors);
  // A board source (helper pipe / LHM) is answering; NVML alone isn't enough.
  $: hasBoard = sensors.some((s) => s.source !== "NVML");
  $: setup = setupState($sensorSetup);
  $: offerSetup =
    !hasBoard &&
    ["enable", "repair", "update", "missing-files"].includes(setup);
  onMount(() => void refreshSensorSetup());
  // Re-check when the temp view opens or a board source appears/disappears.
  $: if (detail === "temp" || hasBoard || !hasBoard) void refreshSensorSetup();
  $: lists = {
    cpu: topCpu ?? topBy(processes, (p) => p.cpu_usage),
    gpu:
      topGpu ??
      topBy(processes, (p) =>
        p.gpu_usage != null && p.gpu_usage > 0 ? p.gpu_usage : null,
      ),
    memory: topMemory ?? topBy(processes, (p) => p.memory_usage),
  };

  const W = 260;
  const H = 36;
  const SLOTS = 40; // 120 s of history at the default 3 s refresh

  $: identity = systemStats.details.identity;
  $: cores = systemStats.cpu_usage;
  $: cpu = averageCpu(cores);
  $: memPct =
    systemStats.memory_total > 0
      ? (systemStats.memory_used / systemStats.memory_total) * 100
      : null;
  $: gpu = gpuSummary(gpus);
  $: gpuId = gpus.find((g) => g.name === gpu.name)?.id;
  $: gpuSeries = lastN(
    history.map((s) => (gpuId ? (s.gpus?.[gpuId] ?? null) : null)),
    SLOTS,
  );
  $: memSeries = lastN(series(history, "memory"), SLOTS);
  $: rxSeries = lastN(series(history, "received"), SLOTS);
  $: txSeries = lastN(series(history, "sent"), SLOTS);
  $: netMax = chartMax([rxSeries, txSeries]);
  $: down = formatRate(systemStats.network_rx_bytes);
  $: up = formatRate(systemStats.network_tx_bytes);
  $: volume = systemVolume(systemStats.details.volumes);
  $: usedPct =
    volume && volume.total > 0
      ? ((volume.total - volume.available) / volume.total) * 100
      : null;
  $: top = topByMemory(processes, 5);
  $: chips = [
    identity.cpu_model,
    formatGB(systemStats.memory_total, 0),
    identity.os_version,
  ].filter(Boolean);
  // Re-evaluated with every new sample (systemStats changes).
  $: since = uptimeInfo(
    systemStats.uptime,
    extra?.started_at,
    systemStats && Date.now() / 1000,
  );
  $: upText = `${since.sinceStart ? "on" : "up"} ${formatDuration(since.seconds)}`;
  $: upTitle = since.sinceStart
    ? `Time since Windows last started (power-on or Fast Startup). Kernel uptime, as Task Manager shows it: ${formatDuration(since.kernel)}`
    : "Time since the system kernel started";

  function level(p: number | null) {
    if (p == null) return "idle";
    if (p >= 85) return "busy";
    if (p >= 50) return "active";
    return "idle";
  }
</script>

<div class="dash" class:compact>
  {#if detail}
    <CardDetail
      kind={detail}
      {systemStats}
      {gpus}
      {history}
      {sensors}
      sources={extra?.sensors.sources ?? []}
      commit={extra?.commit ?? null}
      {lists}
      {compact}
      onBack={() => (detail = null)}
    />
  {:else}
    <header class="dash-header">
      <div class="title" class:hidden={compact}>
        <Activity class="size-5 text-[var(--green)]" />System
      </div>
      <div class="chips">
        {#each chips as chip}<span class="chip">{chip}</span>{/each}
        <span class="chip" title={upTitle}>{upText}</span>
      </div>
    </header>

    <section class="grid" aria-label="System cards">
      <div
        class="card"
        class:clickable={expandable}
        role="button"
        tabindex={expandable ? 0 : -1}
        onclick={() => open("cpu")}
        onkeydown={(e) => openKey(e, "cpu")}
      >
        <div class="card-head">
          <span class="label"><Cpu class="size-3.5" />CPU</span>
          <span class="chip">{cores.length} threads</span>
        </div>
        <div class="big">{formatPercent(cpu)}<small>%</small></div>
        <div class="bars" aria-label="Per-core usage">
          {#each cores as core}
            <span
              class="bar"
              style={`height:${Math.max(6, Math.min(100, core))}%`}
              title={`${core.toFixed(0)}%`}
            ></span>
          {/each}
        </div>
        <div class="foot">
          {level(cpu)} · {identity.physical_cores ?? "?"} cores
        </div>
      </div>

      <div
        class="card"
        class:clickable={expandable}
        role="button"
        tabindex={expandable ? 0 : -1}
        onclick={() => open("gpu")}
        onkeydown={(e) => openKey(e, "gpu")}
      >
        <div class="card-head">
          <span class="label"><Gauge class="size-3.5" />GPU</span>
          {#if gpu.name}<span class="chip ellipsis" title={gpu.name}
              >{gpu.name}</span
            >{/if}
        </div>
        <div class="big">
          {formatPercent(gpu.percent)}{#if gpu.percent != null}<small>%</small
            >{/if}
        </div>
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" class="spark">
          <path
            d={sparklinePath(gpuSeries, {
              width: W,
              height: H,
              max: 100,
              slots: SLOTS,
            })}
            class="line peach"
          />
        </svg>
        <div class="foot">
          {gpu.percent == null
            ? gpus.length
              ? "no live counter for this adapter yet"
              : "no GPU detected"
            : level(gpu.percent)}
        </div>
      </div>

      <div
        class="card"
        class:clickable={expandable}
        role="button"
        tabindex={expandable ? 0 : -1}
        onclick={() => open("memory")}
        onkeydown={(e) => openKey(e, "memory")}
      >
        <div class="card-head">
          <span class="label"><MemoryStick class="size-3.5" />Memory</span>
          <span class="chip"
            >{memPct != null && memPct >= 85 ? "High" : "Normal"}</span
          >
        </div>
        <div class="big">{formatPercent(memPct)}<small>%</small></div>
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" class="spark">
          <path
            d={sparklinePath(memSeries, {
              width: W,
              height: H,
              max: 100,
              slots: SLOTS,
            })}
            class="line yellow"
          />
        </svg>
        <div class="foot">
          {formatGB(systemStats.memory_used, 2)} / {formatGB(
            systemStats.memory_total,
            0,
          )}
        </div>
      </div>

      <div
        class="card"
        class:clickable={expandable}
        role="button"
        tabindex={expandable ? 0 : -1}
        onclick={() => open("disk")}
        onkeydown={(e) => openKey(e, "disk")}
      >
        <div class="card-head">
          <span class="label"><HardDrive class="size-3.5" />Free space</span>
          {#if volume}<span class="chip"
              >{compact ? "" : `${volume.mount} · `}{formatGB(
                volume.total,
                0,
              )}</span
            >{/if}
        </div>
        {#if volume}
          <div class="big">
            {(volume.available / 1024 ** 3).toFixed(0)}<small>GB</small>
          </div>
          <div class="track">
            <span class="fill" style={`width:${usedPct ?? 0}%`}></span>
          </div>
          <div class="foot">
            {formatGB(volume.total - volume.available, 0)} used · {formatPercent(
              usedPct,
            )}%
          </div>
        {:else}
          <div class="big">n/a</div>
          <div class="foot">no volumes reported</div>
        {/if}
      </div>

      <div
        class="card"
        class:clickable={expandable}
        role="button"
        tabindex={expandable ? 0 : -1}
        onclick={() => open("network")}
        onkeydown={(e) => openKey(e, "network")}
      >
        <div class="card-head">
          <span class="label"><Globe class="size-3.5" />Network</span>
          <span class="legend"
            ><i class="dot blue"></i>{compact ? "↓" : "down"}
            <i class="dot green"></i>{compact ? "↑" : "up"}</span
          >
        </div>
        <div class="big">{down.value}<small>{down.unit}</small></div>
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" class="spark">
          <path
            d={sparklinePath(txSeries, {
              width: W,
              height: H,
              max: netMax,
              slots: SLOTS,
            })}
            class="line green"
          />
          <path
            d={sparklinePath(rxSeries, {
              width: W,
              height: H,
              max: netMax,
              slots: SLOTS,
            })}
            class="line blue"
          />
        </svg>
        <div class="foot">↑ {up.value} {up.unit}</div>
      </div>

      <div
        class="card"
        class:clickable={expandable}
        role="button"
        tabindex={expandable ? 0 : -1}
        onclick={() => open("temp")}
        onkeydown={(e) => openKey(e, "temp")}
      >
        <div class="card-head">
          <span class="label"
            ><Thermometer class="size-3.5" />Temp &amp; fan</span
          >
          <span class="chip">{summary.count ? summary.count : "n/a"}</span>
        </div>
        {#if summary.cpuTemp || summary.gpuTemp}
          <div class="big">
            {(summary.cpuTemp ?? summary.gpuTemp)?.value.toFixed(0)}<small
              >°C {summary.cpuTemp ? "CPU" : "GPU"}</small
            >
          </div>
          <div class="foot">
            {[
              summary.cpuTemp && summary.gpuTemp
                ? `GPU ${formatSensor(summary.gpuTemp)}`
                : "",
              fansLine
                ? `${compact ? "" : "Fans "}${fansLine}`
                : summary.fan
                  ? `fan ${formatSensor(summary.fan)}`
                  : "",
            ]
              .filter(Boolean)
              .join(" · ")}
          </div>
        {:else if summary.fan}
          <div class="big">{formatSensor(summary.fan)}</div>
          <div class="foot">{fansLine || summary.fan.name}</div>
        {:else if !offerSetup}
          <div class="big muted">—</div>
          <p class="note">
            {#if compact}No sensor source found. Open for details.{:else}
              No temperature or fan source found. Open this card for details; no
              readings are faked.{/if}
          </p>
        {/if}
        {#if offerSetup}
          <div class="big muted">—</div>
          <span class="enable"
            >{setup === "update"
              ? "Update sensor helper"
              : setup === "repair"
                ? "Repair fan/temperature"
                : "Enable fan/temperature"} ›</span
          >
        {/if}
      </div>
    </section>

    <section class="card procs" aria-label="Top processes by memory">
      <div class="proc-row head">
        <span>Top processes</span><span class="num">Memory</span><span
          class="num">% CPU</span
        >
      </div>
      {#each top as p (p.pid)}
        <div class="proc-row">
          <span class="ellipsis" title={p.command || p.name}>{p.name}</span>
          <span class="num">{formatGB(p.memory_usage, 2)}</span>
          <span class="num">{p.cpu_usage.toFixed(1)}</span>
        </div>
      {/each}
    </section>
  {/if}
  <!-- TODO(winperf): per-process network speed needs ETW (admin); battery,
       health score and speed test are out of scope for v0.2. -->
</div>

<style>
  .dash {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 20px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 980px;
    width: 100%;
    margin: 0 auto;
  }
  .dash > * {
    flex-shrink: 0;
  }
  .dash-header {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 20px;
    font-weight: 650;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    font-size: 11.5px;
    line-height: 1;
    padding: 5px 8px;
    border-radius: 6px;
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
    color: var(--muted-foreground);
    white-space: nowrap;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .enable {
    align-self: flex-start;
    max-width: 100%;
    margin-top: 6px;
    font-size: 11px;
    font-weight: 600;
    line-height: 1.3;
    padding: 3px 9px;
    border-radius: 10px;
    color: #0b0f14;
    background: var(--green, #4ade80);
  }
  .card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    min-height: 132px;
  }
  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }
  .label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--muted-foreground);
  }
  .big {
    font-size: 28px;
    font-weight: 700;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }
  .big small {
    font-size: 13px;
    font-weight: 500;
    margin-left: 4px;
    color: var(--muted-foreground);
  }
  .muted {
    color: var(--muted-foreground);
  }
  .foot {
    margin-top: auto;
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .note {
    font-size: 12px;
    line-height: 1.45;
    color: var(--muted-foreground);
    margin: 0;
  }
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 36px;
  }
  .bar {
    flex: 1;
    min-width: 2px;
    border-radius: 2px;
    background: var(--green);
    opacity: 0.85;
  }
  .spark {
    width: 100%;
    height: 36px;
    overflow: visible;
  }
  .line {
    fill: none;
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  .line.blue {
    stroke: var(--blue);
  }
  .line.green {
    stroke: var(--green);
  }
  .line.yellow {
    stroke: var(--yellow);
  }
  .line.peach {
    stroke: var(--peach);
  }
  .track {
    height: 10px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 10%, transparent);
    overflow: hidden;
    margin: 12px 0 6px;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 999px;
    background: var(--blue);
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--muted-foreground);
  }
  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .dot.blue {
    background: var(--blue);
  }
  .dot.green {
    background: var(--green);
    margin-left: 4px;
  }
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .procs {
    min-height: 0;
    gap: 0;
    padding: 6px 16px;
  }
  .proc-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 110px 70px;
    gap: 12px;
    padding: 6px 0;
    font-size: 13.5px;
    font-weight: 550;
  }
  .proc-row.head {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--muted-foreground);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .hidden {
    display: none;
  }
  .card.clickable {
    cursor: pointer;
    transition:
      border-color 120ms,
      background 120ms;
  }
  .card.clickable:hover,
  .card.clickable:focus-visible {
    border-color: color-mix(in oklab, var(--foreground) 30%, var(--border));
    outline: none;
  }
  /* winperf mini window: same cards, squeezed into ~340 px. */
  .dash.compact {
    flex: none;
    overflow: visible;
    padding: 0;
    gap: 8px;
    max-width: none;
  }
  .compact .chip {
    font-size: 10.5px;
    padding: 4px 6px;
  }
  .compact .chips {
    gap: 4px;
  }
  .compact .grid {
    gap: 8px;
  }
  .compact .card {
    min-height: 0;
    padding: 10px 11px;
    gap: 4px;
    border-radius: 12px;
  }
  .compact .label {
    font-size: 12px;
    white-space: nowrap;
  }
  .compact .big {
    font-size: 22px;
  }
  .compact .big small {
    font-size: 11px;
    margin-left: 3px;
  }
  .compact .bars,
  .compact .spark {
    height: 26px;
  }
  .compact .track {
    height: 8px;
    margin: 9px 0 5px;
  }
  .compact .foot,
  .compact .note {
    font-size: 11px;
  }
  .compact .card-head .chip {
    max-width: 92px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .compact .legend {
    font-size: 10.5px;
    gap: 4px;
  }
  .compact .procs {
    padding: 4px 11px;
  }
  .compact .proc-row {
    grid-template-columns: minmax(0, 1fr) 72px 44px;
    gap: 8px;
    padding: 5px 0;
    font-size: 12.5px;
  }
  .compact .proc-row.head {
    font-size: 11.5px;
  }
</style>
