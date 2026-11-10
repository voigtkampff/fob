<script lang="ts">
  // Physical disk health in the Disk detail. Asks the backend when shown and
  // every 5 minutes while shown; the backend caches for 5 minutes too.
  import { invoke } from "@tauri-apps/api/core";
  import type { DiskHealthReport, PhysicalDisk } from "$lib/types";
  import {
    BUS_NAMES,
    diskAlert,
    driveWarnings,
    formatCount,
    formatDecimalBytes,
    operationalBad,
    queryNote,
    watchDiskHealth,
    type DiskHealthView,
  } from "$lib/utils/disk-health.js";
  import { t } from "$lib/i18n";

  let view = $state<DiskHealthView>({ kind: "loading" });
  let pending = $state(false);
  $effect(() =>
    watchDiskHealth(
      () => invoke<DiskHealthReport>("disk_health"),
      (next, refreshing) => {
        view = next;
        pending = refreshing;
      },
    ),
  );

  const bus = (d: PhysicalDisk) => BUS_NAMES[d.bus] ?? $t(`dh.bus.${d.bus}`);
  const diskName = (d: PhysicalDisk) =>
    d.number == null ? $t("dh.diskNoNumber") : $t("dh.disk", { n: d.number });
  const time = (ms: number) =>
    new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const hex = (code: number) => (code >>> 0).toString(16).toUpperCase();
</script>

<div class="panel health">
  <div class="row">
    <h3 id="disk-health-title" tabindex="-1" class="name strong">
      {$t("dh.title")}
    </h3>
    {#if pending}<span class="muted small">{$t("dh.pending")}</span>{/if}
  </div>
  {#if view.kind === "loading" || view.kind === "pending"}
    <p class="note">{$t("dh.loading")}</p>
  {:else if view.kind === "failed"}
    <p class="note">{$t("dh.failed", { v: view.message })}</p>
  {:else if view.kind === "unavailable"}
    <p class="note">{$t("dh.windowsOnly")}</p>
  {:else if view.kind === "ok"}
    {#each view.disks as d}
      {@const note = queryNote(d)}
      <div class="disk" class:alert={diskAlert(d)}>
        <div class="row">
          <span class="name strong">{diskName(d)}</span>
          <span class="muted small tag"
            >{bus(d)}{d.removable ? ` · ${$t("dh.removable")}` : ""}</span
          >
        </div>
        {#if d.model}<div class="muted small model" title={d.model}>
            {d.model}
          </div>{/if}
        <div class="muted small">
          {d.volumes.length ? d.volumes.join(" · ") : $t("dh.noLetter")}
        </div>
        <dl>
          <dt>{$t("dh.windows")}</dt>
          {#if d.windows.kind === "ok"}
            <dd
              class:bad={d.windows.health === "warning" ||
                d.windows.health === "unhealthy"}
            >
              {$t(
                `dh.health.${d.windows.health}`,
              )}{#each d.windows.operational.filter((o) => o.kind !== "ok") as o}<span
                  class:bad={operationalBad(o)}
                  >{" · "}{o.kind === "other"
                    ? $t("dh.op.other", { v: o.code })
                    : $t(`dh.op.${o.kind}`)}</span
                >{/each}
            </dd>
          {:else if d.windows.kind === "not_reported"}
            <dd class="muted">{$t("dh.windowsNotReported")}</dd>
          {:else}
            <dd class="muted">
              {$t("dh.windowsFailed", { v: hex(d.windows.code) })}
            </dd>
          {/if}
          {#if d.nvme.kind === "ok"}
            {@const log = d.nvme.log}
            <dt>{$t("dh.temp")}</dt>
            <dd>
              {log.temperature_c == null ? "—" : `${log.temperature_c} °C`}
            </dd>
            <dt title={$t("dh.usedNote")}>{$t("dh.used")}</dt>
            <dd title={$t("dh.usedNote")}>{log.percent_used}%</dd>
            <dt>{$t("dh.spare")}</dt>
            <dd>
              {log.available_spare == null
                ? "—"
                : log.spare_threshold == null
                  ? `${log.available_spare}%`
                  : $t("dh.spareValue", {
                      v: `${log.available_spare}%`,
                      t: `${log.spare_threshold}%`,
                    })}
            </dd>
            <dt>{$t("dh.written")}</dt>
            <dd>{formatDecimalBytes(log.data_written_bytes)}</dd>
            <dt>{$t("dh.read")}</dt>
            <dd>{formatDecimalBytes(log.data_read_bytes)}</dd>
            <dt>{$t("dh.powerOn")}</dt>
            <dd>{$t("dh.hours", { n: formatCount(log.power_on_hours) })}</dd>
            <dt>{$t("dh.cycles")}</dt>
            <dd>{formatCount(log.power_cycles)}</dd>
            <dt>{$t("dh.unsafe")}</dt>
            <dd>{formatCount(log.unsafe_shutdowns)}</dd>
            <dt>{$t("dh.mediaErrors")}</dt>
            <dd class:bad={log.media_errors > 0}>
              {formatCount(log.media_errors)}
            </dd>
            <dt>{$t("dh.errorLog")}</dt>
            <dd>{formatCount(log.error_log_entries)}</dd>
            {#if log.warning_temp_minutes > 0}
              <dt>{$t("dh.hotMinutes")}</dt>
              <dd>
                {$t("dh.minutes", { n: formatCount(log.warning_temp_minutes) })}
              </dd>
            {/if}
          {/if}
          {#if d.ata.kind === "ok"}
            {@const s = d.ata.stats}
            {#if s.temperature_c != null}
              <dt>{$t("dh.temp")}</dt>
              <dd>{s.temperature_c} °C</dd>
            {/if}
            {#if s.percent_used != null}
              <dt title={$t("dh.usedNote")}>{$t("dh.used")}</dt>
              <dd title={$t("dh.usedNote")}>{s.percent_used}%</dd>
            {/if}
            {#if s.data_written_bytes != null}
              <dt>{$t("dh.written")}</dt>
              <dd>{formatDecimalBytes(s.data_written_bytes)}</dd>
            {/if}
            {#if s.data_read_bytes != null}
              <dt>{$t("dh.read")}</dt>
              <dd>{formatDecimalBytes(s.data_read_bytes)}</dd>
            {/if}
            {#if s.power_on_hours != null}
              <dt>{$t("dh.powerOn")}</dt>
              <dd>{$t("dh.hours", { n: formatCount(s.power_on_hours) })}</dd>
            {/if}
            {#if s.power_cycles != null}
              <dt>{$t("dh.cycles")}</dt>
              <dd>{formatCount(s.power_cycles)}</dd>
            {/if}
          {/if}
        </dl>
        {#if driveWarnings(d).length}
          <div class="warnings" role="status">
            <span class="small strong">{$t("dh.warnTitle")}</span>
            {#each driveWarnings(d) as key}<span class="small"
                >{$t(`dh.warn.${key}`)}</span
              >{/each}
          </div>
        {/if}
        {#if note}
          <p class="note">{$t(note.key, note.vars)}</p>
        {/if}
      </div>
    {:else}
      <p class="note">{$t("dh.none")}</p>
    {/each}
    <p class="note">{$t("dh.checked", { v: time(view.checked_at_ms) })}</p>
  {/if}
</div>

<style>
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
  h3 {
    margin: 0;
    font-size: inherit;
    outline: none;
  }
  .strong {
    font-weight: 600;
  }
  .muted {
    color: var(--muted-foreground);
  }
  .small {
    font-size: 11.5px;
  }
  .tag {
    flex-shrink: 0;
  }
  .model {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .disk {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 0 6px;
    border-top: 1px solid var(--border);
  }
  .disk.alert {
    border-top-color: var(--peach);
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    column-gap: 16px;
    row-gap: 3px;
    margin: 4px 0 0;
    font-size: 12px;
  }
  dt {
    color: var(--muted-foreground);
  }
  dd {
    margin: 0;
    text-align: right;
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
  .bad {
    color: var(--peach);
  }
  .warnings {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 8px;
    border-radius: 8px;
    color: var(--peach);
    background: color-mix(in oklab, var(--peach) 12%, transparent);
  }
  .note {
    font-size: 11.5px;
    line-height: 1.45;
    color: var(--muted-foreground);
    margin: 0;
  }
</style>
