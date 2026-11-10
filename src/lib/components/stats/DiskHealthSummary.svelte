<script lang="ts">
  // Physical disks in the storage side panel: one line per disk with the
  // status, temperature and life used it reports, and a link to the full
  // Disk detail. Reads only while the panel is open.
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "$lib/components/ui/button/index.js";
  import { HeartPulse } from "@lucide/svelte";
  import DetailSection from "./DetailSection.svelte";
  import type { DiskHealthReport, PhysicalDisk } from "$lib/types";
  import {
    BUS_NAMES,
    diskAlert,
    driveWarnings,
    headline,
    operationalBad,
    watchDiskHealth,
    type DiskHealthView,
  } from "$lib/utils/disk-health.js";
  import { t } from "$lib/i18n";

  let { onDetails }: { onDetails: () => void } = $props();
  let view = $state<DiskHealthView>({ kind: "loading" });
  $effect(() =>
    watchDiskHealth(
      () => invoke<DiskHealthReport>("disk_health"),
      (next) => (view = next),
    ),
  );

  const name = (d: PhysicalDisk) =>
    d.number == null ? $t("dh.diskNoNumber") : $t("dh.disk", { n: d.number });
  const bus = (d: PhysicalDisk) => BUS_NAMES[d.bus] ?? $t(`dh.bus.${d.bus}`);
  const hex = (code: number) => (code >>> 0).toString(16).toUpperCase();
  function status(d: PhysicalDisk) {
    if (driveWarnings(d).length) return $t("dh.driveWarning");
    if (d.windows.kind === "not_reported") return $t("dh.windowsNotReported");
    if (d.windows.kind === "failed")
      return $t("dh.windowsFailed", { v: hex(d.windows.code) });
    const bad = d.windows.operational.find(operationalBad);
    const health = $t(`dh.health.${d.windows.health}`);
    return bad && bad.kind !== "ok" && bad.kind !== "other"
      ? `${health} · ${$t(`dh.op.${bad.kind}`)}`
      : health;
  }
  /** Only what the drive reported: its NVMe log or ATA statistics. */
  function facts(d: PhysicalDisk) {
    const h = headline(d);
    const out: string[] = [];
    if (h.temperature_c != null)
      out.push(`${$t("dh.temp")} ${h.temperature_c} °C`);
    if (h.percent_used != null)
      out.push(`${$t("dh.usedShort")} ${h.percent_used}%`);
    return out;
  }
</script>

<DetailSection title={$t("dh.title")}>
  {#snippet action()}<Button
      variant="outline"
      size="sm"
      class="h-7 shrink-0 text-xs"
      onclick={onDetails}><HeartPulse />{$t("dh.viewDetails")}</Button
    >{/snippet}
  {#if view.kind === "loading" || view.kind === "pending"}
    <p class="text-sm text-muted-foreground">{$t("dh.loading")}</p>
  {:else if view.kind === "failed"}
    <p class="text-sm text-muted-foreground">
      {$t("dh.failed", { v: view.message })}
    </p>
  {:else if view.kind === "unavailable"}
    <p class="text-sm text-muted-foreground">{$t("dh.windowsOnly")}</p>
  {:else}
    {#each view.disks as d}
      {@const alert = diskAlert(d)}
      {@const lines = facts(d)}
      {@const where = [
        d.removable ? `${bus(d)} · ${$t("dh.removable")}` : bus(d),
        d.model,
        d.volumes.length ? d.volumes.join(" · ") : $t("dh.noLetter"),
      ]
        .filter(Boolean)
        .join(" · ")}
      <div
        class="disk space-y-1 border-b border-border pb-3 last:border-0 last:pb-0"
      >
        <div class="flex items-baseline justify-between gap-3 text-sm">
          <span class="name min-w-0 truncate font-medium">{name(d)}</span>
          <span
            class={[
              "status shrink-0 text-xs tabular-nums",
              alert && "text-[var(--peach)]",
              !alert && d.windows.kind !== "ok" && "text-muted-foreground",
            ]}>{status(d)}</span
          >
        </div>
        <p class="truncate text-xs text-muted-foreground" title={where}>
          {where}
        </p>
        {#if lines.length}<p
            class="facts text-xs text-muted-foreground tabular-nums"
            title={$t("dh.usedNote")}
          >
            {lines.join(" · ")}
          </p>{/if}
      </div>
    {:else}
      <p class="text-sm text-muted-foreground">{$t("dh.none")}</p>
    {/each}
  {/if}
</DetailSection>
