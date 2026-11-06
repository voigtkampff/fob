import { derived, writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import type { SensorSetupStatus } from "$lib/types";
import {
  createSensorSetupController,
  type Lifecycle,
} from "$lib/utils/sensor-setup.js";

export const sensorLifecycle = writable<Lifecycle>({
  status: null,
  activity: { kind: "idle" },
  feedback: { kind: "none" },
});
export const sensorSetup = derived(sensorLifecycle, (state) => state.status);
export const sensorSetupController = createSensorSetupController({
  read: () => invoke<SensorSetupStatus>("sensor_setup_status"),
  change: (enable) => invoke<SensorSetupStatus>("sensor_setup", { enable }),
  publish: (state) => sensorLifecycle.set(state),
  delay: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
});
export const refreshSensorSetup = sensorSetupController.refresh;
