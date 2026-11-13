import test from "node:test";
import assert from "node:assert/strict";
import { mergeHistory } from "../src/lib/utils/history-transfer.js";
const sample = (timestamp, cpu = 0) => ({
  timestamp,
  interval: 3000,
  cpu,
  memory: 20,
  received: 0,
  sent: 0,
  gpus: {},
  cpu_temp: null,
  gpu_temp: null,
  disk_read: 0,
  disk_write: 0,
});

test("initial full history, repeated polls, rollover and hidden-window catch-up", () => {
  let history = mergeHistory([], {
    reset: true,
    samples: [sample(1000), sample(4000)],
    first_timestamp: 1000,
  });
  assert.deepEqual(
    mergeHistory(history, { reset: false, samples: [], first_timestamp: 1000 }),
    history,
  );
  history = mergeHistory(history, {
    reset: false,
    samples: [sample(7000), sample(10000)],
    first_timestamp: 4000,
  });
  assert.deepEqual(
    history.map((s) => s.timestamp),
    [4000, 7000, 10000],
  );
  history = mergeHistory(history, {
    reset: true,
    samples: [sample(200000), sample(203000)],
    first_timestamp: 200000,
  });
  assert.deepEqual(
    history.map((s) => s.timestamp),
    [200000, 203000],
  );
});

test("restart, replacement, rollback and gap markers keep native sample values", () => {
  const old = [sample(1000), sample(4000)];
  const replacement = sample(4000, null);
  assert.deepEqual(
    mergeHistory(old, {
      reset: true,
      samples: [old[0], replacement],
      first_timestamp: 1000,
    }),
    [old[0], replacement],
  );
  assert.deepEqual(
    mergeHistory(old, {
      reset: true,
      samples: [sample(500)],
      first_timestamp: 500,
    }),
    [sample(500)],
  );
  assert.deepEqual(
    mergeHistory(old, { reset: true, samples: [], first_timestamp: null }),
    [],
  );
});
