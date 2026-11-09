// Fob: performance / efficiency core groups for the CPU views. The backend
// reads which logical processors belong to each group once (Windows
// efficiency class); each refresh averages the group's own threads.
import { averageCpu } from "./dashboard.js";

/**
 * @typedef {{ kind: "performance" | "efficiency", cores: number, threads: number[] }} TopologyGroup
 * @typedef {{ state: "unknown" } | { state: "uniform" }
 *   | { state: "hybrid", vendor: "intel" | "amd" | "other", groups: TopologyGroup[] }} CpuTopology
 * @typedef {{ kind: "performance" | "efficiency", label: string, cores: number,
 *   threads: number[], usage: number | null }} CpuGroup
 */

/**
 * The groups to show, performance first, or null for no split: a uniform
 * CPU, an unread layout, or one that does not cover exactly the threads in
 * `usage`. A group's usage is the average of its threads (0-100), so a
 * P-core group with 16 threads on 8 cores counts each thread once, like the
 * total. null when a thread has no reading (shown as "—", never 0%).
 * @param {CpuTopology | undefined | null} topology
 * @param {number[]} usage
 * @returns {CpuGroup[] | null}
 */
export function cpuGroups(topology, usage) {
  if (topology?.state !== "hybrid" || topology.groups.length < 2) return null;
  const covered = topology.groups.flatMap((g) => g.threads);
  const unique = new Set(covered);
  if (
    covered.length !== usage.length ||
    unique.size !== usage.length ||
    covered.some((i) => !Number.isInteger(i) || i < 0 || i >= usage.length) ||
    topology.groups.some((g) => !g.threads.length)
  )
    return null;
  const names = topology.vendor === "intel" ? LABELS.intel : LABELS.other;
  return [...topology.groups]
    .sort((a, b) => kindOrder(a.kind) - kindOrder(b.kind))
    .map((g) => ({
      kind: g.kind,
      label: names[g.kind],
      cores: g.cores,
      threads: g.threads,
      usage: averageCpu(g.threads.map((i) => usage[i])),
    }));
}

/** Intel's own names, P-core / E-core; AMD and others get generic names. */
const LABELS = {
  intel: { performance: "cpu.pCores", efficiency: "cpu.eCores" },
  other: { performance: "cpu.perfCores", efficiency: "cpu.effCores" },
};

/** @param {string} kind */
const kindOrder = (kind) => (kind === "performance" ? 0 : 1);

/** "42" for a reading, "—" for none. @param {number | null} value */
export const groupPercent = (value) =>
  value == null ? "—" : String(Math.round(value));
