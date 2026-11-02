<script lang="ts">
  // winperf: enable/disable the read-only fan & temperature helper. Nothing
  // runs until the user has read what will happen and pressed Continue.
  import type { SensorSetupStatus } from "$lib/types";
  import { setupState, setupSteps } from "$lib/utils/dashboard.js";
  import { changeSensorSetup } from "$lib/stores/sensorSetup";
  import { t } from "$lib/i18n";

  export let status: SensorSetupStatus | null;

  let confirm: "enable" | "disable" | null = null;
  let busy = false;
  let error = "";

  $: state = setupState(status);
  $: steps = status ? setupSteps(status, $t) : [];
  $: action =
    state === "update"
      ? $t("dash.updateHelper")
      : state === "repair" || state === "starting"
        ? $t("dash.repair")
        : $t("dash.enable");

  async function run(enable: boolean) {
    busy = true;
    error = "";
    try {
      await changeSensorSetup(enable);
      confirm = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if state === "missing-files"}
  <div class="setup">
    <!-- Fixed strings from the locale tables (only <b>), no user data. -->
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    <p class="note">{@html $t("setup.missingFiles")}</p>
  </div>
{:else if confirm === "enable" && status}
  <div class="setup">
    <p class="lead">{action}</p>
    <p class="note">{$t("setup.willDo")}</p>
    <ol class="steps">
      {#each steps as step}<li>{step}</li>{/each}
    </ol>
    <p class="note">{$t("setup.uacNote")}</p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="buttons">
      <button class="primary" disabled={busy} onclick={() => run(true)}
        >{busy ? $t("setup.waiting") : $t("setup.continue")}</button
      >
      <button disabled={busy} onclick={() => ((confirm = null), (error = ""))}
        >{$t("setup.cancel")}</button
      >
    </div>
  </div>
{:else if confirm === "disable"}
  <div class="setup">
    <p class="lead">{$t("setup.disableTitle")}</p>
    <p class="note">{$t("setup.disableNote")}</p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="buttons">
      <button class="primary" disabled={busy} onclick={() => run(false)}
        >{busy ? $t("setup.waiting") : $t("setup.disable")}</button
      >
      <button disabled={busy} onclick={() => ((confirm = null), (error = ""))}
        >{$t("setup.cancel")}</button
      >
    </div>
  </div>
{:else if state === "enable" || state === "repair" || state === "update"}
  <div class="setup">
    <p class="note">
      {state === "enable"
        ? $t("setup.needHelper")
        : state === "update"
          ? $t("setup.newer")
          : $t("setup.incomplete")}
    </p>
    <div class="buttons">
      <button class="primary" onclick={() => (confirm = "enable")}
        >{action}…</button
      >
      {#if state !== "enable"}<button onclick={() => (confirm = "disable")}
          >{$t("setup.disableMore")}</button
        >{/if}
    </div>
  </div>
{:else if state === "starting" || state === "ready"}
  <div class="setup row">
    <p class="note">
      {state === "ready" ? $t("setup.running") : $t("setup.starting")}
    </p>
    <div class="buttons">
      {#if state === "starting"}<button onclick={() => (confirm = "enable")}
          >{$t("setup.repairMore")}</button
        >{/if}
      <button onclick={() => (confirm = "disable")}
        >{$t("setup.disableMore")}</button
      >
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
  .setup.row {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    column-gap: 12px;
    padding: 8px 12px;
  }
  .setup.row .note {
    flex: 1 1 12rem;
    min-width: 0;
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
