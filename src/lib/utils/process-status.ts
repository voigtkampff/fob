// Fob: process status as shown (the backend reports sysinfo's English
// names; filters keep comparing those, the UI shows the locale's word).
import type { Translate } from "$lib/i18n";

const KEYS: Record<string, string> = {
  Running: "status.running",
  Sleeping: "status.sleeping",
  Idle: "status.idle",
  Stopped: "status.stopped",
  Zombie: "status.zombie",
  Unknown: "status.unknown",
};
/** The statuses the filter offers, in order. */
export const FILTER_STATUSES = Object.keys(KEYS);

export function statusLabel(status: string, t: Translate): string {
  const key =
    KEYS[status] ?? KEYS[status.replace(/^\w/, (c) => c.toUpperCase())];
  return key ? t(key) : status;
}
