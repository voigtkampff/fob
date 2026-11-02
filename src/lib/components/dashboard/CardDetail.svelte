<script lang="ts">
  // winperf: one detail view for any card (back button / Esc returns).
  import type {
    GpuDevice,
    ResourceSample,
    Sensor,
    SystemStats,
  } from "$lib/types";
  import type { DetailKind } from "./detail";
  import {
    averageCpu,
    chartMax,
    formatGB,
    formatPercent,
    formatRate,
    formatSensor,
    groupByHardware,
    lastN,
    series,
    sparklinePath,
  } from "$lib/utils/dashboard.js";
  import { ChevronLeft } from "@lucide/svelte";

  type Row = {
    pid: number;
    name: string;
    cpu_usage: number;
    memory_usage: number;
    gpu_usage?: number | null;
  };
  export let kind: DetailKind;
  export let systemStats: SystemStats;
  export let gpus: GpuDevice[] = [];
  export let history: ResourceSample[] = [];
  export let sensors: Sensor[] = [];
  export let sources: string[] = [];
  export let commit: [number, number] | null = null;
  export let lists: { cpu: Row[]; gpu: Row[]; memory: Row[] };
  export let compact = false;
  export let onBack: () => void;

  const titles: Record<DetailKind, string> = {
    cpu: "CPU",
    gpu: "GPU",
    memory: "Memory",
    disk: "Drives",
    network: "Network",
    temp: "Temperatures & fans",
  };
  const W = 300;
  const H = 70;
  const SLOTS = 40;
  $: n = compact ? 8 : 10;
  $: cores = systemStats.cpu_usage;
  $: rx = lastN(series(history, "received"), SLOTS);
  $: tx = lastN(series(history, "sent"), SLOTS);
  $: netMax = chartMax([rx, tx]);
  $: cpuLine = lastN(series(history, "cpu"), SLOTS);
  $: memLine = lastN(series(history, "memory"), SLOTS);
  $: interfaces = [...systemStats.details.interfaces]
    .filter((i) => i.received_total + i.sent_total > 0)
    .sort(
      (a, b) =>
        (b.received ?? 0) +
          (b.sent ?? 0) -
          ((a.received ?? 0) + (a.sent ?? 0)) ||
        b.received_total - a.received_total,
    );
  $: volumes = [...systemStats.details.volumes].sort((a, b) =>
    a.mount.localeCompare(b.mount),
  );
  $: groups = groupByHardware(sensors);
  $: memAvail = systemStats.memory_total - systemStats.memory_used;
  const pct = (part: number, total: number) =>
    total > 0 ? Math.min(100, Math.max(0, (part / total) * 100)) : 0;
  const rate = (v: number | null | undefined) => {
    const r = formatRate(v);
    return `${r.value} ${r.unit}`.trim();
  };
</script>

<section class="detail" class:compact aria-label={`${titles[kind]} details`}>
  <header class="head">
    <button class="back" onclick={onBack} aria-label="Back to cards"
      ><ChevronLeft class="size-4" />Back</button
    >
    <h2>{titles[kind]}</h2>
  </header>

  {#if kind === "cpu"}
    <div class="panel">
      <div class="row">
        <span class="big"
          >{formatPercent(averageCpu(cores))}<small>%</small></span
        >
        <span class="muted"
          >{systemStats.details.identity.cpu_model ?? ""} · {systemStats.details
            .identity.physical_cores ?? "?"} cores / {cores.length} threads</span
        >
      </div>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" class="graph">
        <path
          d={sparklinePath(cpuLine, {
            width: W,
            height: H,
            max: 100,
            slots: SLOTS,
          })}
          class="line green"
        />
      </svg>
      <div class="cores">
        {#each cores as core, i}
          <div class="core" title={`Thread ${i}: ${core.toFixed(0)}%`}>
            <div class="core-bar">
              <span style={`height:${Math.max(3, Math.min(100, core))}%`}
              ></span>
            </div>
            <small>{i}</small>
          </div>
        {/each}
      </div>
    </div>
    <div class="panel">
      <div class="list-head"><span>Top processes</span><span>% CPU</span></div>
      {#each lists.cpu.slice(0, n) as p (p.pid)}
        <div class="item">
          <span class="name">{p.name}</span><span class="num"
            >{p.cpu_usage.toFixed(1)}</span
          >
        </div>
      {/each}
    </div>
  {:else if kind === "gpu"}
    {#each gpus as g (g.id)}
      <div class="panel">
        <div class="row">
          <span class="name strong">{g.name}</span>
          <span class="num strong"
            >{g.utilization == null
              ? "n/a"
              : `${g.utilization.toFixed(0)} %`}</span
          >
        </div>
        <div class="muted small">
          {g.memory_total
            ? `${g.memory_used != null ? formatGB(g.memory_used, 1) + " / " : ""}${formatGB(g.memory_total, 0)} ${g.memory_kind}`
            : g.memory_kind}
          {g.temperature != null ? ` · ${g.temperature.toFixed(0)} °C` : ""}
          {g.fan_percent != null ? ` · fan ${g.fan_percent.toFixed(0)} %` : ""}
        </div>
        {#each g.engines ?? [] as [engine, load]}
          <div class="meter">
            <span class="label">{engine}</span>
            <div class="track"><span style={`width:${load}%`}></span></div>
            <span class="num">{load.toFixed(0)}%</span>
          </div>
        {/each}
      </div>
    {:else}
      <p class="note">No GPU detected.</p>
    {/each}
    <div class="panel">
      <div class="list-head"><span>Top processes</span><span>% GPU</span></div>
      {#each lists.gpu.slice(0, n) as p (p.pid)}
        <div class="item">
          <span class="name">{p.name}</span><span class="num"
            >{p.gpu_usage?.toFixed(1)}</span
          >
        </div>
      {:else}
        <p class="note">
          No process is using the GPU right now, or this system doesn't report
          per-process GPU use (Windows: GPU Engine counters).
        </p>
      {/each}
    </div>
  {:else if kind === "memory"}
    <div class="panel">
      <div class="row">
        <span class="big"
          >{formatPercent(
            pct(systemStats.memory_used, systemStats.memory_total),
          )}<small>%</small></span
        >
      </div>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" class="graph">
        <path
          d={sparklinePath(memLine, {
            width: W,
            height: H,
            max: 100,
            slots: SLOTS,
          })}
          class="line yellow"
        />
      </svg>
      <div class="kv">
        <span>Used</span><span>{formatGB(systemStats.memory_used, 2)}</span>
      </div>
      <div class="kv">
        <span>Available</span><span>{formatGB(memAvail, 2)}</span>
      </div>
      <div class="kv">
        <span>Total</span><span>{formatGB(systemStats.memory_total, 1)}</span>
      </div>
      {#if commit}
        <div class="kv">
          <span>Committed</span><span
            >{formatGB(commit[0], 1)} / {formatGB(commit[1], 1)}</span
          >
        </div>
      {/if}
      {#if systemStats.details.swap_total > 0}
        <div class="kv">
          <span>{commit ? "Page file" : "Swap"}</span><span
            >{formatGB(systemStats.details.swap_used, 1)} / {formatGB(
              systemStats.details.swap_total,
              1,
            )}</span
          >
        </div>
      {/if}
    </div>
    <div class="panel">
      <div class="list-head"><span>Top processes</span><span>Memory</span></div>
      {#each lists.memory.slice(0, n) as p (p.pid)}
        <div class="item">
          <span class="name">{p.name}</span><span class="num"
            >{formatGB(p.memory_usage, 2)}</span
          >
        </div>
      {/each}
    </div>
  {:else if kind === "disk"}
    <div class="panel">
      {#each volumes as v (v.mount)}
        <div class="drive">
          <div class="row">
            <span class="name strong">{v.mount}</span>
            <span class="muted small"
              >{formatGB(v.available, 0)} free of {formatGB(v.total, 0)}</span
            >
          </div>
          <div class="track wide">
            <span style={`width:${pct(v.total - v.available, v.total)}%`}
            ></span>
          </div>
          <div class="muted small">
            {v.name || v.file_system}{v.file_system && v.name
              ? ` · ${v.file_system}`
              : ""}{v.read_only ? " · read-only" : ""}
          </div>
        </div>
      {:else}
        <p class="note">No drives reported.</p>
      {/each}
    </div>
  {:else if kind === "network"}
    <div class="panel">
      <div class="row rates">
        <div>
          <span class="muted small"><i class="dot blue"></i>Download</span>
          <div class="big">{rate(systemStats.network_rx_bytes)}</div>
        </div>
        <div>
          <span class="muted small"><i class="dot green"></i>Upload</span>
          <div class="big">{rate(systemStats.network_tx_bytes)}</div>
        </div>
      </div>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" class="graph">
        <path
          d={sparklinePath(tx, {
            width: W,
            height: H,
            max: netMax,
            slots: SLOTS,
          })}
          class="line green"
        />
        <path
          d={sparklinePath(rx, {
            width: W,
            height: H,
            max: netMax,
            slots: SLOTS,
          })}
          class="line blue"
        />
      </svg>
    </div>
    <div class="panel">
      <div class="list-head"><span>Adapter</span><span>↓ / ↑</span></div>
      {#each interfaces as i (i.name)}
        <div class="item">
          <span class="name">{i.name}</span><span class="num"
            >{rate(i.received)} / {rate(i.sent)}</span
          >
        </div>
      {:else}
        <p class="note">No active network adapters.</p>
      {/each}
      <p class="note">
        Per-app network speed needs administrator rights (ETW tracing), so it
        isn't shown.
      </p>
    </div>
  {:else if kind === "temp"}
    {#each groups as group (group.hardware)}
      <div class="panel">
        <div class="list-head"><span>{group.hardware}</span></div>
        {#each group.sensors as s}
          <div class="item">
            <span class="name">{s.name}</span><span class="num"
              >{formatSensor(s)}</span
            >
          </div>
        {/each}
      </div>
    {:else}
      <div class="panel">
        <p class="note">
          No temperature or fan readings are available right now. winperf
          doesn't install a driver of its own, so CPU, motherboard and fan
          sensors only appear when a supported hardware-monitoring source is
          running on this PC.
        </p>
        <p class="note">
          NVIDIA GPUs report their temperature and fan through the driver and
          appear here automatically. No numbers are faked.
        </p>
      </div>
    {/each}
    {#if sources.length}
      <p class="note">Source: {sources.join(", ")}</p>
    {/if}
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .head h2 {
    font-size: 15px;
    font-weight: 650;
    margin: 0;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 12px;
    padding: 4px 8px 4px 4px;
    border-radius: 8px;
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
    color: var(--foreground);
  }
  .back:hover {
    background: color-mix(in oklab, var(--foreground) 14%, transparent);
  }
  .panel {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }
  .rates {
    justify-content: flex-start;
    gap: 28px;
  }
  .big {
    font-size: 26px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .big small {
    font-size: 12px;
    margin-left: 3px;
    color: var(--muted-foreground);
  }
  .muted {
    color: var(--muted-foreground);
  }
  .small,
  .muted {
    font-size: 11.5px;
  }
  .strong {
    font-weight: 600;
    font-size: 13px;
  }
  .graph {
    width: 100%;
    height: 70px;
    border-radius: 6px;
    background: color-mix(in oklab, var(--foreground) 4%, transparent);
  }
  .line {
    fill: none;
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: round;
  }
  .line.green {
    stroke: var(--green);
  }
  .line.blue {
    stroke: var(--blue);
  }
  .line.yellow {
    stroke: var(--yellow);
  }
  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    margin-right: 5px;
  }
  .dot.blue {
    background: var(--blue);
  }
  .dot.green {
    background: var(--green);
  }
  .cores {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(18px, 1fr));
    gap: 4px;
  }
  .core {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .core-bar {
    width: 100%;
    height: 40px;
    display: flex;
    align-items: flex-end;
    border-radius: 3px;
    background: color-mix(in oklab, var(--foreground) 7%, transparent);
    overflow: hidden;
  }
  .core-bar span {
    width: 100%;
    background: var(--green);
  }
  .core small {
    font-size: 9px;
    color: var(--muted-foreground);
  }
  .list-head {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    color: var(--muted-foreground);
  }
  .item {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12.5px;
    padding: 2px 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .num {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .kv {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
  }
  .kv span:first-child {
    color: var(--muted-foreground);
  }
  .meter {
    display: grid;
    grid-template-columns: 90px 1fr 36px;
    align-items: center;
    gap: 8px;
    font-size: 11.5px;
  }
  .meter .label {
    color: var(--muted-foreground);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meter .num {
    text-align: right;
  }
  .track {
    height: 6px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 10%, transparent);
    overflow: hidden;
  }
  .track.wide {
    height: 8px;
  }
  .track span {
    display: block;
    height: 100%;
    background: var(--blue);
    border-radius: 999px;
  }
  .meter .track span {
    background: var(--peach);
  }
  .drive {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 4px 0;
  }
  .note {
    font-size: 11.5px;
    line-height: 1.45;
    color: var(--muted-foreground);
    margin: 0;
  }
  .compact .big {
    font-size: 22px;
  }
  .compact .graph {
    height: 56px;
  }
</style>
