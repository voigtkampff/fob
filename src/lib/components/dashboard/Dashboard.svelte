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
  import { MINI, fixedRows } from "$lib/utils/mini-layout.js";
  import { onMount } from "svelte";
  import { t } from "$lib/i18n";
  import ChartCursor from "$lib/components/charts/ChartCursor.svelte";
  import { percentText as pct1 } from "$lib/utils/chart-cursor.js";
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
  /** Opens the disk space map (a button in the disk detail). */
  export let onSpace: (() => void) | null = null;

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

  $: identity = systemStats.details.identity;
  $: cores = systemStats.cpu_usage;
  $: cpu = averageCpu(cores);
  $: memPct =
    systemStats.memory_total > 0
      ? (systemStats.memory_used / systemStats.memory_total) * 100
      : null;
  $: gpu = gpuSummary(gpus);
  $: gpuId = gpus.find((g) => g.name === gpu.name)?.id;
  $: pickGpu = (s: ResourceSample) =>
    gpuId ? (s.gpus?.[gpuId] ?? null) : null;
  $: netMax = chartMax([series(history, "received"), series(history, "sent")]);
  const rateText = (v: number) => {
    const r = formatRate(v);
    return `${r.value} ${r.unit}`.trim();
  };
  $: down = formatRate(systemStats.network_rx_bytes);
  $: up = formatRate(systemStats.network_tx_bytes);
  $: volume = systemVolume(systemStats.details.volumes);
  $: usedPct =
    volume && volume.total > 0
      ? ((volume.total - volume.available) / volume.total) * 100
      : null;
  $: top = topByMemory(processes, MINI.procRows);
  // The mini window always shows exactly MINI.procRows rows (fixed height).
  $: topRows = compact ? fixedRows(top, MINI.procRows) : top;
  $: chips = [
    identity.cpu_model,
    formatGB(systemStats.memory_total, 0),
    identity.os_version,
  ].filter(Boolean);
  // Re-evaluated with every new sample (systemStats changes).
  $: since = uptimeInfo(
    systemStats.uptime,
    extra?.started?.[0],
    systemStats && Date.now() / 1000,
  );
  $: upText = $t(since.sinceStart ? "uptime.on" : "uptime.up", {
    d: formatDuration(since.seconds, $t),
  });
  $: upTitle = since.sinceStart
    ? $t("uptime.titleStart", {
        d: formatDuration(since.kernel, $t),
        how: $t(`uptime.kind.${extra?.started?.[1] ?? "cold"}`),
      })
    : $t("uptime.titleKernel");

  function level(p: number | null) {
    if (p == null) return $t("level.idle");
    if (p >= 85) return $t("level.busy");
    if (p >= 50) return $t("level.active");
    return $t("level.idle");
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
      netApps={extra?.net_apps ?? null}
      {lists}
      {compact}
      onBack={() => (detail = null)}
      {onSpace}
    />
  {:else}
    <header class="dash-header">
      <div class="title" class:hidden={compact}>
        <Activity class="size-5 text-[var(--green)]" />{$t("dash.system")}
      </div>
      <div class="chips">
        {#each chips as chip}<span class="chip">{chip}</span>{/each}
        <span class="chip" title={upTitle}>{upText}</span>
      </div>
    </header>

    <section class="grid" aria-label={$t("dash.cards")}>
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
          <span class="chip">{$t("dash.threads", { n: cores.length })}</span>
        </div>
        <div class="big">{formatPercent(cpu, $t)}<small>%</small></div>
        <div class="bars" aria-label={$t("dash.perCore")}>
          {#each cores as core}
            <span
              class="bar"
              style={`height:${Math.max(6, Math.min(100, core))}%`}
              title={`${core.toFixed(0)}%`}
            ></span>
          {/each}
        </div>
        <div class="foot">
          {level(cpu)} · {$t("dash.cores", {
            n: identity.physical_cores ?? "?",
          })}
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
          {formatPercent(gpu.percent, $t)}{#if gpu.percent != null}<small
              >%</small
            >{/if}
        </div>
        <ChartCursor
          {history}
          max={100}
          format={pct1}
          lines={[{ pick: pickGpu, color: "var(--peach)" }]}
        >
          <svg
            viewBox={`0 0 ${W} ${H}`}
            preserveAspectRatio="none"
            class="spark"
          >
            <path
              d={sparklinePath(history, pickGpu, {
                width: W,
                height: H,
                max: 100,
              })}
              class="line peach"
            />
          </svg>
        </ChartCursor>
        <div class="foot">
          {gpu.percent == null
            ? gpus.length
              ? $t("dash.gpuNoCounter")
              : $t("dash.noGpu")
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
          <span class="label"
            ><MemoryStick class="size-3.5" />{$t("dash.memory")}</span
          >
          <span class="chip"
            >{memPct != null && memPct >= 85
              ? $t("dash.memHigh")
              : $t("dash.memNormal")}</span
          >
        </div>
        <div class="big">{formatPercent(memPct, $t)}<small>%</small></div>
        <ChartCursor
          {history}
          max={100}
          format={pct1}
          lines={[{ pick: (s) => s.memory, color: "var(--yellow)" }]}
        >
          <svg
            viewBox={`0 0 ${W} ${H}`}
            preserveAspectRatio="none"
            class="spark"
          >
            <path
              d={sparklinePath(history, (s) => s.memory, {
                width: W,
                height: H,
                max: 100,
              })}
              class="line yellow"
            />
          </svg>
        </ChartCursor>
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
          <span class="label"
            ><HardDrive class="size-3.5" />{$t("dash.freeSpace")}</span
          >
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
            {$t("dash.used", {
              v: formatGB(volume.total - volume.available, 0),
            })} · {formatPercent(usedPct, $t)}%
          </div>
        {:else}
          <div class="big">{$t("common.na")}</div>
          <div class="foot">{$t("dash.noVolumes")}</div>
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
          <span class="label"
            ><Globe class="size-3.5" />{$t("dash.network")}</span
          >
          <span class="legend"
            ><i class="dot blue"></i>{compact ? "↓" : $t("dash.down")}
            <i class="dot green"></i>{compact ? "↑" : $t("dash.up")}</span
          >
        </div>
        <div class="big">{down.value}<small>{down.unit}</small></div>
        <ChartCursor
          {history}
          max={netMax}
          format={rateText}
          lines={[
            { pick: (s) => s.received, color: "var(--blue)", label: "↓" },
            { pick: (s) => s.sent, color: "var(--green)", label: "↑" },
          ]}
        >
          <svg
            viewBox={`0 0 ${W} ${H}`}
            preserveAspectRatio="none"
            class="spark"
          >
            <path
              d={sparklinePath(history, (s) => s.sent, {
                width: W,
                height: H,
                max: netMax,
              })}
              class="line green"
            />
            <path
              d={sparklinePath(history, (s) => s.received, {
                width: W,
                height: H,
                max: netMax,
              })}
              class="line blue"
            />
          </svg>
        </ChartCursor>
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
            ><Thermometer class="size-3.5" />{$t("dash.temp")}</span
          >
          <span class="chip"
            >{summary.count ? summary.count : $t("common.na")}</span
          >
        </div>
        {#if summary.cpuTemp || summary.gpuTemp}
          <div class="big">
            {(summary.cpuTemp ?? summary.gpuTemp)?.value.toFixed(0)}<small
              >°C {summary.cpuTemp ? "CPU" : "GPU"}</small
            >
          </div>
          <div class="foot wrap2">
            {[
              summary.cpuTemp && summary.gpuTemp
                ? `GPU ${formatSensor(summary.gpuTemp)}`
                : "",
              fansLine
                ? compact
                  ? fansLine
                  : $t("dash.fans", { v: fansLine })
                : summary.fan
                  ? $t("dash.fan", { v: formatSensor(summary.fan) })
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
            {compact ? $t("dash.noSensorShort") : $t("dash.noSensor")}
          </p>
        {/if}
        {#if offerSetup}
          <div class="big muted">—</div>
          <span class="enable"
            >{setup === "update"
              ? $t("dash.updateHelper")
              : setup === "repair"
                ? $t("dash.repair")
                : $t("dash.enable")} ›</span
          >
        {/if}
      </div>
    </section>

    <section class="card procs" aria-label={$t("dash.topByMemory")}>
      <div class="proc-row head">
        <span>{$t("dash.topProcs")}</span><span class="num"
          >{$t("dash.memory")}</span
        ><span class="num">{$t("dash.cpuPct")}</span>
      </div>
      {#each topRows as p, i (p?.pid ?? `empty-${i}`)}
        {#if p}
          <div class="proc-row">
            <span class="ellipsis" title={p.command || p.name}>{p.name}</span>
            <span class="num">{formatGB(p.memory_usage, 2)}</span>
            <span class="num">{p.cpu_usage.toFixed(1)}</span>
          </div>
        {:else}
          <div class="proc-row" aria-hidden="true"></div>
        {/if}
      {/each}
    </section>
  {/if}
  <!-- TODO(winperf): battery, health score and speed test are out of scope. -->
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
    text-align: center;
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
  /* Fob mini window: same cards, squeezed into ~340 px. Every block of the
     cards view has a fixed height (utils/mini-layout.js), so the window has
     one size for every view and the cards never overflow it. */
  .dash.compact {
    flex: none;
    overflow: visible;
    padding: 0;
    gap: var(--mini-gap, 8px);
    max-width: none;
  }
  .compact .chip {
    font-size: 10.5px;
    line-height: var(--mini-chipRow, 18px);
    height: var(--mini-chipRow, 18px);
    padding: 0 6px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    box-sizing: border-box;
  }
  .compact .chips {
    gap: 4px;
    height: var(--mini-chips, 40px);
    overflow: hidden;
    align-content: flex-start;
  }
  .compact .grid {
    gap: var(--mini-cardGap, 8px);
    grid-auto-rows: var(--mini-card, 118px);
  }
  .compact .card {
    min-height: 0;
    height: var(--mini-card, 118px);
    box-sizing: border-box;
    overflow: hidden;
    padding: 10px 11px;
    gap: 4px;
    border-radius: 12px;
  }
  .compact .grid .foot {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 0;
  }
  /* The temperature card has no graph: its fan line may take two lines. */
  .compact .grid .foot.wrap2,
  .compact .grid .note {
    white-space: normal;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
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
    padding: var(--mini-procPad, 4px) 11px;
    box-sizing: border-box;
    height: var(--mini-procs, 169px);
    gap: 0;
    overflow: hidden;
  }
  .compact .proc-row {
    grid-template-columns: minmax(0, 1fr) 72px 44px;
    gap: 8px;
    padding: 0;
    height: var(--mini-procRow, 27px);
    align-items: center;
    font-size: 12.5px;
  }
  .compact .proc-row.head {
    height: var(--mini-procHead, 24px);
    font-size: 11.5px;
  }
</style>
