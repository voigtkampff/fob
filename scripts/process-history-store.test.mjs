import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import ts from "typescript";
import { get } from "svelte/store";
const dataUrl = (s) =>
  `data:text/javascript;base64,${Buffer.from(s).toString("base64")}`;
let source = await readFile(
  new URL("../src/lib/stores/processes.ts", import.meta.url),
  "utf8",
);
const utils = dataUrl(
  ts.transpileModule(
    await readFile(
      new URL("../src/lib/utils/index.ts", import.meta.url),
      "utf8",
    ),
    { compilerOptions: { module: ts.ModuleKind.ES2022 } },
  ).outputText,
);
source = source
  .replaceAll('"$lib/utils"', JSON.stringify(utils))
  .replaceAll(
    '"$lib/utils/history-transfer.js"',
    JSON.stringify(
      new URL("../src/lib/utils/history-transfer.js", import.meta.url).href,
    ),
  )
  .replaceAll(
    '"svelte/store"',
    JSON.stringify(import.meta.resolve("svelte/store")),
  )
  .replaceAll(
    '"@tauri-apps/api/core"',
    JSON.stringify(
      dataUrl(
        "export const invoke = (...args) => globalThis.__historyInvoke(...args)",
      ),
    ),
  );
const { processStore } = await import(
  dataUrl(
    ts.transpileModule(source, {
      compilerOptions: {
        module: ts.ModuleKind.ES2022,
        target: ts.ScriptTarget.ES2022,
      },
    }).outputText,
  )
);

test("store acknowledges only applied history and coalesces overlapping polls", async () => {
  let calls = [];
  let resolve;
  globalThis.__historyInvoke = (cmd, args) => {
    calls.push([cmd, args]);
    return new Promise((r) => (resolve = r));
  };
  const a = processStore.getProcesses();
  const b = processStore.getProcesses();
  assert.equal(a, b);
  assert.equal(calls.length, 1);
  assert.equal(calls[0][1].historyCursor, null);
  const response = (history) => ({
    frame: { processes: [], stats: {}, gpus: [], captured_at: 4000 },
    history,
    paused: false,
    error: null,
  });
  const sample = { timestamp: 1000, cpu: 0 };
  const cursor = { epoch: "one", timestamp: 1000 };
  resolve(
    response({ reset: true, samples: [sample], cursor, first_timestamp: 1000 }),
  );
  await a;
  assert.deepEqual(get(processStore).resourceHistory, [sample]);
  const c = processStore.getProcesses();
  assert.deepEqual(calls[1][1].historyCursor, cursor);
  resolve(
    response({
      reset: false,
      samples: [{ timestamp: 4000, cpu: 10 }],
      cursor: { epoch: "one", timestamp: 4000 },
      first_timestamp: 1000,
    }),
  );
  await c;
  assert.deepEqual(
    get(processStore).resourceHistory.map((s) => s.cpu),
    [0, 10],
  );
  globalThis.__historyInvoke = async () => {
    throw new Error("synthetic read failure");
  };
  await processStore.getProcesses();
  assert.deepEqual(get(processStore).historyCursor, {
    epoch: "one",
    timestamp: 4000,
  });
  assert.deepEqual(
    get(processStore).resourceHistory.map((s) => s.cpu),
    [0, 10],
  );
  delete globalThis.__historyInvoke;
});
