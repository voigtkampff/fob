// Deterministic synthetic JSON byte counts, not a speed or disk-write benchmark.
import assert from "node:assert/strict";
import { mergeHistory } from "../src/lib/utils/history-transfer.js";
const sample = (i) => ({
  timestamp: 1700000000000 + i * 3000,
  interval: 3000,
  cpu: 12.5 + i / 10,
  memory: 42.5,
  received: 128000,
  sent: 64000,
  gpus: { "fixture-gpu-a": 23, "fixture-gpu-b": 0 },
  cpu_temp: 51,
  gpu_temp: 40,
  disk_read: 1048576,
  disk_write: 4096,
});
const history = Array.from({ length: 41 }, (_, i) => sample(i + 1));
const previous = Array.from({ length: 41 }, (_, i) => sample(i));
const cursor = {
  epoch: "1700000000000000000-0",
  timestamp: history.at(-1).timestamp,
};
const update = {
  reset: false,
  samples: history.slice(-1),
  cursor,
  first_timestamp: history[0].timestamp,
};
assert.deepEqual(mergeHistory(previous, update), history);
const bytes = (v) => Buffer.byteLength(JSON.stringify(v));
const historyBytes = { full: bytes(history), incremental: bytes(update) };
const reports = [];
for (const count of [50, 200, 500]) {
  const processes = Array.from({ length: count }, (_, i) => ({
    pid: 100 + i,
    ppid: 4,
    name: `fixture-${i}.exe`,
    command: `fixture-${i}.exe --worker`,
    user: "fixture-account",
    start_time: 1699990000 + i,
    run_time: 10000,
    status: "Running",
    cpu_usage: i % 8 ? 0 : 1.5,
    memory_usage: 40000000,
    virtual_memory: 100000000,
    threads: null,
    environ: Array.from(
      { length: 8 },
      (_, j) => `FIXTURE_${j}=synthetic-value-not-a-private-configuration`,
    ),
    root: "",
    disk_read: 0,
    disk_write: 4096,
    gpu_usage: 0,
    gpu_memory: 0,
    session_id: 1,
    ports: [],
  }));
  const frame = {
    sequence: 42,
    captured_at: history.at(-1).timestamp,
    processes,
    stats: {
      cpu_usage: Array(16).fill(12.5),
      memory_total: 32000000000,
      memory_used: 14000000000,
    },
    gpus: [],
    extra: { sensors: { sensors: [] }, commit: null },
  };
  const common = {
    port_error: null,
    frame,
    paused: false,
    interval: 3000,
    error: null,
  };
  const runs = Array.from({ length: 5 }, () => ({
    before: bytes({ ...common, history }),
    after: bytes({ ...common, history: update }),
  }));
  for (const r of runs) assert.deepEqual(r, runs[0]);
  assert.ok(runs[0].after < runs[0].before);
  reports.push({
    processes: count,
    runs,
    initialResetBytes: bytes({
      ...common,
      history: { ...update, reset: true, samples: history },
    }),
    reductionPercent: +(100 * (1 - runs[0].after / runs[0].before)).toFixed(2),
  });
}
assert.ok(historyBytes.incremental < historyBytes.full * 0.2);
console.log(
  JSON.stringify(
    {
      measurement:
        "UTF-8 JSON bytes, five identical alternating before/after serializations; synthetic data, no timing or disk-write claim",
      historyBytes,
      requestCursorBytes: bytes({ historyCursor: cursor }) - bytes({}),
      reports,
      errors: 0,
      reconstruction: "exact",
    },
    null,
    2,
  ),
);
