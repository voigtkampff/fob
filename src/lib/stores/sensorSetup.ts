// winperf: status of the optional fan/temperature helper (Windows only).
import { get, writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import type { SensorSetupStatus } from "$lib/types";

export const sensorSetup = writable<SensorSetupStatus | null>(null);

let pending: Promise<void> | null = null;
export function refreshSensorSetup() {
  pending ??= invoke<SensorSetupStatus>("sensor_setup_status")
    .then((s) => sensorSetup.set(s))
    .catch(() => {})
    .finally(() => (pending = null));
  return pending;
}

/** Runs the elevated setup/removal (one UAC prompt). Throws the message to show. */
export async function changeSensorSetup(enable: boolean) {
  const status = await invoke<SensorSetupStatus>("sensor_setup", { enable });
  sensorSetup.set(status);
  // The helper needs a few seconds to open the hardware and answer.
  for (let i = 0; enable && i < 8 && !get(sensorSetup)?.running; i++) {
    await new Promise((r) => setTimeout(r, 1500));
    await refreshSensorSetup();
  }
}
