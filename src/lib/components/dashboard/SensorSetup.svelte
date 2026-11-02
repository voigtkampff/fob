<script lang="ts">
  // winperf: enable/disable the read-only fan & temperature helper. Nothing
  // runs until the user has read what will happen and pressed Continue.
  import type { SensorSetupStatus } from "$lib/types";
  import { setupState, setupSteps } from "$lib/utils/dashboard.js";
  import { changeSensorSetup } from "$lib/stores/sensorSetup";

  export let status: SensorSetupStatus | null;

  let confirm: "enable" | "disable" | null = null;
  let busy = false;
  let error = "";

  $: state = setupState(status);
  $: steps = status ? setupSteps(status) : [];
  $: action =
    state === "update"
      ? "Update sensor helper"
      : state === "repair" || state === "starting"
        ? "Repair fan/temperature"
        : "Enable fan/temperature";

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
    <p class="note">
      Fan and temperature readings need the <b>sensors</b> and <b>PawnIO</b>
      folders that came with winperf. Keep the whole winperf folder together, next
      to winperf.exe.
    </p>
  </div>
{:else if confirm === "enable" && status}
  <div class="setup">
    <p class="lead">{action}</p>
    <p class="note">winperf will do the following:</p>
    <ol class="steps">
      {#each steps as step}<li>{step}</li>{/each}
    </ol>
    <p class="note">
      Windows shows one permission (UAC) prompt for all of it; winperf itself
      keeps running without administrator rights. The helper only reads sensors,
      never changes fan speeds, and works alongside FanControl or other
      monitoring tools. You can undo it here with Disable.
    </p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="buttons">
      <button class="primary" disabled={busy} onclick={() => run(true)}
        >{busy ? "Waiting for Windows…" : "Continue"}</button
      >
      <button disabled={busy} onclick={() => ((confirm = null), (error = ""))}
        >Cancel</button
      >
    </div>
  </div>
{:else if confirm === "disable"}
  <div class="setup">
    <p class="lead">Disable fan/temperature</p>
    <p class="note">
      Stops the helper, deletes the “winperf-sensors” scheduled task and removes
      C:\Program Files\winperf\sensors. Windows shows one permission prompt. The
      PawnIO driver stays installed; other tools such as FanControl may use it.
      You can uninstall it from Settings › Apps.
    </p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="buttons">
      <button class="primary" disabled={busy} onclick={() => run(false)}
        >{busy ? "Waiting for Windows…" : "Disable"}</button
      >
      <button disabled={busy} onclick={() => ((confirm = null), (error = ""))}
        >Cancel</button
      >
    </div>
  </div>
{:else if state === "enable" || state === "repair" || state === "update"}
  <div class="setup">
    <p class="note">
      {#if state === "enable"}CPU, motherboard and fan sensors need winperf's
        read-only sensor helper. It is set up once, with your permission.
      {:else if state === "update"}This winperf comes with a newer sensor helper
        than the installed one.
      {:else}The sensor helper setup is incomplete or points somewhere else.{/if}
    </p>
    <div class="buttons">
      <button class="primary" onclick={() => (confirm = "enable")}
        >{action}…</button
      >
      {#if state !== "enable"}<button onclick={() => (confirm = "disable")}
          >Disable…</button
        >{/if}
    </div>
  </div>
{:else if state === "starting" || state === "ready"}
  <div class="setup row">
    <p class="note">
      {state === "ready"
        ? "Sensor helper is running."
        : "Sensor helper is installed but not answering yet; winperf asked it to start."}
    </p>
    <div class="buttons">
      {#if state === "starting"}<button onclick={() => (confirm = "enable")}
          >Repair…</button
        >{/if}
      <button onclick={() => (confirm = "disable")}>Disable…</button>
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
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
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
  .buttons {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }
  button {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 7px;
    border: 1px solid color-mix(in oklab, var(--foreground) 16%, transparent);
    background: transparent;
    color: var(--foreground);
    cursor: pointer;
    white-space: nowrap;
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
