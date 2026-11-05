<script lang="ts">
  import {
    SERIES_COLOR,
    STROKE_WIDTH,
    DASHED,
    FILL_TOP,
    FILL_BOTTOM,
  } from "$lib/utils/chart-style.js";
  import * as Chart from "$lib/components/ui/chart/index.js";
  import { AreaChart, Area, LinearGradient, Points } from "layerchart";
  import type { ResourceSample } from "$lib/types";
  import { formatBytes } from "$lib/utils";
  import ChartCursor from "$lib/components/charts/ChartCursor.svelte";
  import { percentText } from "$lib/utils/chart-cursor.js";
  import {
    RESOURCE_WINDOW_MS,
    networkCeiling,
    nearestResourceSample,
  } from "$lib/utils/resource-history.js";
  import { clockTime } from "$lib/utils/chart-cursor.js";
  import { t } from "$lib/i18n";
  type Metric = "cpu" | "memory" | "network" | "gpu" | "temp" | "disk";
  type Key =
    | "cpu"
    | "memory"
    | "received"
    | "sent"
    | "gpu"
    | "cpu_temp"
    | "gpu_temp"
    | "disk_read"
    | "disk_write";
  let {
    history: rawHistory = [],
    gpuId = "",
    unsupported = false,
    expanded = false,
    metric,
    label,
    isFrozen = false,
    unavailable = false,
  }: {
    history: ResourceSample[];
    gpuId?: string;
    unsupported?: boolean;
    expanded?: boolean;
    metric: Metric;
    label: string;
    isFrozen?: boolean;
    unavailable?: boolean;
  } = $props();
  const history = $derived(
    rawHistory.map((p) => ({ ...p, gpu: p.gpus?.[gpuId] ?? null })),
  );
  let selectedStamp = $state<number | null>(null);
  let keyboard = $state(false);
  // Byte-rate charts: two lines, a solid one in and a dashed one out.
  const isRate = $derived(metric === "network" || metric === "disk");
  const series = $derived(
    metric === "disk"
      ? [
          {
            key: "disk_read" as Key,
            label: $t("stats.read"),
            color: SERIES_COLOR.disk_read,
          },
          {
            key: "disk_write" as Key,
            label: $t("stats.write"),
            color: SERIES_COLOR.disk_write,
          },
        ]
      : metric === "network"
        ? [
            {
              key: "received" as Key,
              label: $t("stats.received"),
              color: SERIES_COLOR.received,
            },
            {
              key: "sent" as Key,
              label: $t("stats.sent"),
              color: SERIES_COLOR.sent,
            },
          ]
        : metric === "temp"
          ? [
              {
                key: "cpu_temp" as Key,
                label: "CPU",
                color: SERIES_COLOR.cpu_temp,
              },
              {
                key: "gpu_temp" as Key,
                label: "GPU",
                color: SERIES_COLOR.gpu_temp,
              },
            ]
          : [
              {
                key: metric as Key,
                label:
                  metric === "cpu"
                    ? "CPU"
                    : metric === "gpu"
                      ? "GPU"
                      : $t("stats.ramUsed"),
                color: SERIES_COLOR[metric],
              },
            ],
  );
  const config = $derived(
    Object.fromEntries(
      series.map((s) => [s.key, { label: s.label, color: s.color }]),
    ) satisfies Chart.ChartConfig,
  );
  const end = $derived(history.at(-1)?.timestamp ?? 0);
  const ceiling = $derived(
    metric === "disk"
      ? networkCeiling(history, ["disk_read", "disk_write"])
      : metric === "network"
        ? networkCeiling(history)
        : metric === "temp"
          ? Math.max(
              100,
              ...history.flatMap((p) => [p.cpu_temp ?? 0, p.gpu_temp ?? 0]),
            )
          : 100,
  );
  const selected = $derived(
    selectedStamp === null
      ? history.at(-1)
      : nearestResourceSample(history, selectedStamp),
  );
  const selectedIndex = $derived(
    selected ? history.findIndex((p) => p.timestamp === selected.timestamp) : 0,
  );
  const validCount = $derived(
    history.filter((p) => series.some((s) => p[s.key] != null)).length,
  );
  const time = clockTime;
  const value = (n: number | null) =>
    n === null
      ? $t("common.na")
      : isRate
        ? `${formatBytes(n)}/s`
        : metric === "temp"
          ? `${n.toFixed(0)} °C`
          : `${n.toFixed(1)}%`;
  const describe = (p: ResourceSample | undefined) =>
    p
      ? `${time(p.timestamp)} · ${series.map((s) => `${s.label} ${value(p[s.key] ?? null)}`).join(" · ")}`
      : $t("trend.collecting");
  const caption = $derived(
    selectedStamp !== null
      ? metric === "network" && selected
        ? `↓ ${value(selected.received)} · ↑ ${value(selected.sent)}`
        : describe(selected)
      : isFrozen
        ? $t("mini.paused")
        : unavailable
          ? $t("trend.unavailable")
          : unsupported
            ? metric === "temp"
              ? $t("trend.noSensor")
              : $t("trend.noCounter")
            : validCount < 2
              ? $t("trend.collecting")
              : isRate
                ? `2m · 0–${formatBytes(ceiling)}/s`
                : metric === "temp"
                  ? `2m · 0–${ceiling.toFixed(0)} °C`
                  : "2m · 0–100%",
  );
  const KEYS = ["ArrowLeft", "ArrowRight", "Home", "End", "Escape"]; // i18n-ignore
  function key(event: KeyboardEvent) {
    if (!KEYS.includes(event.key)) return;
    event.preventDefault();
    if (event.key === "Escape") {
      selectedStamp = null;
      keyboard = false;
      return;
    }
    keyboard = true;
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? history.length - 1
          : Math.max(
              0,
              Math.min(
                history.length - 1,
                selectedIndex + (event.key === "ArrowLeft" ? -1 : 1),
              ),
            );
    selectedStamp = history[next]?.timestamp ?? null;
  }
</script>

<div class="mt-auto min-w-0 space-y-1">
  <ChartCursor
    {history}
    end={end || undefined}
    windowMs={RESOURCE_WINDOW_MS}
    max={ceiling}
    format={(n) =>
      isRate
        ? `${formatBytes(n)}/s`
        : metric === "temp"
          ? `${n.toFixed(0)} °C`
          : percentText(n)}
    lines={series.map((s) => ({
      pick: (p: ResourceSample) => p[s.key] ?? null,
      color: s.color,
      label:
        metric === "network"
          ? s.key === "received"
            ? "↓"
            : "↑"
          : metric === "temp" || metric === "disk"
            ? s.label
            : undefined,
    }))}
    inset={{ left: 2, right: 3, top: 3, bottom: 2 }}
  >
    <Chart.Container
      {config}
      class={`${expanded ? "h-28" : "h-12"} w-full rounded-sm aspect-auto focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring`}
      role="slider"
      tabindex={history.length ? 0 : -1}
      aria-label={$t("trend.aria", { label })}
      aria-valuemin={0}
      aria-valuemax={Math.max(0, history.length - 1)}
      aria-valuenow={selectedIndex}
      aria-valuetext={describe(selected)}
      aria-disabled={!history.length}
      onkeydown={key}
      onfocus={() => {
        keyboard = true;
        selectedStamp = history.at(-1)?.timestamp ?? null;
      }}
      onblur={() => {
        keyboard = false;
        selectedStamp = null;
      }}
    >
      {#if history.length}
        <AreaChart
          data={history}
          x="timestamp"
          y={series[0].key}
          {series}
          seriesLayout="overlap"
          xDomain={[end - RESOURCE_WINDOW_MS, end]}
          yDomain={[0, ceiling]}
          yNice={false}
          axis={false}
          grid={false}
          rule={false}
          legend={false}
          highlight={false}
          tooltipContext={false}
          motion={{ type: "none" }}
          padding={{ left: 2, right: 3, top: 3, bottom: 2 }}
        >
          {#snippet marks()}
            {#each series as s}
              <LinearGradient vertical>
                {#snippet stopsContent()}<stop
                    offset="0%"
                    stop-color={s.color}
                    stop-opacity={FILL_TOP}
                  /><stop
                    offset="100%"
                    stop-color={s.color}
                    stop-opacity={FILL_BOTTOM}
                  />{/snippet}
                {#snippet children({ gradient })}<Area
                    seriesKey={s.key}
                    fill={gradient}
                    fillOpacity={1}
                    defined={(d: ResourceSample) => d[s.key] != null}
                    line={{
                      stroke: s.color,
                      "stroke-width": STROKE_WIDTH,
                      "stroke-dasharray": DASHED.has(s.key) ? "3 2" : undefined,
                      "stroke-linecap": "round",
                      "stroke-linejoin": "round",
                    }}
                    motion={{ type: "none" }}
                  />{/snippet}
              </LinearGradient>
              {#if selected && selected[s.key] != null}<Points
                  data={[selected]}
                  x="timestamp"
                  y={s.key}
                  r={2}
                  fill={s.color}
                  stroke="var(--card)"
                  strokeWidth={1}
                />{/if}
            {/each}
          {/snippet}
        </AreaChart>
      {:else}<div class="flex h-full w-full items-end">
          <div class="w-full border-b border-border/60"></div>
        </div>{/if}
    </Chart.Container>
  </ChartCursor>
  <div
    class="min-h-3 truncate text-[10px] leading-3 text-muted-foreground tabular-nums"
    title={$t("trend.title", { label, caption })}
  >
    {caption}
  </div>
</div>
