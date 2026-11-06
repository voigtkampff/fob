<script lang="ts">
  import { onMount } from "svelte";
  import type { SensorSetupStatus } from "$lib/types";
  import { setupSteps } from "$lib/utils/dashboard.js";
  import { helperView } from "$lib/utils/sensor-setup.js";
  import {
    sensorLifecycle,
    sensorSetupController,
    refreshSensorSetup,
  } from "$lib/stores/sensorSetup";
  import { t } from "$lib/i18n";

  export let status: SensorSetupStatus | null;
  export let details = false;
  $: view = helperView(status, feedback);
  $: activity = $sensorLifecycle.activity;
  $: feedback = $sensorLifecycle.feedback;
  $: busy = activity.kind === "working" || !!status?.busy;
  $: confirming =
    activity.kind === "confirm" || activity.kind === "working"
      ? activity
      : null;
  $: steps = status ? setupSteps(status, $t) : [];
  $: action = $t(
    view.kind === "install"
      ? "helper.install"
      : view.kind === "update"
        ? "helper.update"
        : "helper.repair",
  );
  onMount(() => {
    void refreshSensorSetup();
    const timer = setInterval(refreshSensorSetup, 3000);
    return () => clearInterval(timer);
  });
</script>

<svelte:window onfocus={refreshSensorSetup} />

{#if view.kind !== "unsupported"}
  <div class="setup" aria-busy={busy}>
    {#if details}
      <p class="note">{$t("helper.benefit")}</p>
      <p class="note">{$t("helper.privacy")}</p>
      <div class="versions note">
        <span
          >{$t("helper.installedVersion")}: {status?.installed_version ??
            $t("helper.unknownVersion")}</span
        >
        <span
          >{$t("helper.bundledVersion")}: {status?.bundled_version ??
            $t("helper.unknownVersion")}</span
        >
      </div>
    {/if}
    <p class="lead" role="status">
      {busy ? $t("setup.waiting") : $t(`helper.state.${view.kind}`)}
    </p>
    {#if view.kind === "missing-files"}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <p class="note">{@html $t("setup.missingFiles")}</p>
    {/if}
    {#if confirming?.enable && status}
      <p class="note">{$t("setup.willDo")}</p>
      <ol class="steps">
        {#each steps as step}<li>{step}</li>{/each}
      </ol>
      <p class="note">{$t("setup.uacNote")}</p>
    {:else if confirming && !confirming.enable}
      <p class="lead">{$t("setup.disableTitle")}</p>
      <p class="note">{$t("setup.disableNote")}</p>
    {/if}
    {#if feedback.kind === "error"}
      <p class="error" role="alert">
        {$t(
          feedback.message === "busy" ? "helper.busy" : "helper.failed",
        )}{feedback.message !== "busy" ? ` ${feedback.message}` : ""}
      </p>
    {:else if feedback.kind === "cancelled"}
      <p class="note" role="status">{$t("helper.cancelled")}</p>
    {:else if feedback.kind === "success"}
      <p class="note" role="status">{$t("helper.completed")}</p>
    {/if}
    <div class="buttons">
      {#if confirming}
        <button
          class="primary"
          disabled={busy}
          onclick={sensorSetupController.run}
          >{busy ? $t("setup.waiting") : $t("setup.continue")}</button
        >
        <button disabled={busy} onclick={sensorSetupController.dismiss}
          >{$t("setup.cancel")}</button
        >
      {:else}
        {#if view.enable}<button
            class="primary"
            disabled={busy}
            onclick={() => sensorSetupController.request(true)}
            >{action}…</button
          >{/if}
        {#if view.disable}<button
            disabled={busy}
            onclick={() => sensorSetupController.request(false)}
            >{$t("setup.disableMore")}</button
          >{/if}
        {#if details || feedback.kind === "error" || view.kind === "stopped"}
          <button
            disabled={busy || activity.kind === "checking"}
            onclick={refreshSensorSetup}>{$t("helper.refresh")}</button
          >
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .setup {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: 10px;
    border: 1px solid color-mix(in oklab, var(--foreground) 12%, transparent);
    background: color-mix(in oklab, var(--foreground) 4%, transparent);
  }
  .versions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    overflow-wrap: anywhere;
  }
  .lead {
    font-weight: 600;
    font-size: 13px;
  }
  .note,
  .steps {
    font-size: 12px;
    line-height: 1.45;
    color: var(--muted-foreground);
  }
  .steps {
    padding-left: 18px;
    list-style: decimal;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .error {
    overflow-wrap: anywhere;
    font-size: 12px;
    color: var(--red, #f87171);
  }
  /* Buttons wrap instead of overflowing the narrow mini window. */
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    max-width: 100%;
  }
  button {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 7px;
    border: 1px solid color-mix(in oklab, var(--foreground) 16%, transparent);
    background: transparent;
    color: var(--foreground);
    cursor: pointer;
    max-width: 100%;
    text-align: left;
    overflow-wrap: anywhere;
  }
  button:hover:not(:disabled) {
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
  }
  button.primary {
    background: var(--green, #4ade80);
    border-color: transparent;
    color: #0b0f14;
    font-weight: 600;
  }
  button.primary:hover:not(:disabled) {
    background: color-mix(in oklab, var(--green, #4ade80) 85%, white);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
