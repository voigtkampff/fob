import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  diskAlert,
  driveWarnings,
  formatCount,
  formatDecimalBytes,
  headline,
  operationalBad,
  queryNote,
  PENDING_RETRY_MS,
  REFRESH_MS,
  watchDiskHealth,
} from "../src/lib/utils/disk-health.js";

const rust = await readFile(
  new URL("../src-tauri/src/disk_health/mod.rs", import.meta.url),
  "utf8",
);
const types = await readFile(
  new URL("../src/lib/types/index.ts", import.meta.url),
  "utf8",
);
const snake = (name) => name.replace(/(?<!^)([A-Z])/g, "_$1").toLowerCase();
/** Variant names of a Rust enum, as serde's snake_case writes them. */
function variants(name) {
  const body = rust.match(
    new RegExp(`pub enum ${name} \\{([\\s\\S]*?)\\n\\}`),
  )[1];
  return [...body.matchAll(/^ {4}([A-Z]\w*)/gm)].map((m) => snake(m[1])).sort();
}
/** String literals of a TS union or kind list. */
function literals(source) {
  return [...source.matchAll(/"(\w+)"/g)].map((m) => m[1]).sort();
}
const typeBody = (name) =>
  types.match(new RegExp(`export type ${name} =([\\s\\S]*?);\\n(?=\\S|$)`))[1];

test("the TS types name every variant the Rust report can send", () => {
  assert.deepEqual(literals(typeBody("DiskBus")), variants("Bus"));
  assert.deepEqual(literals(typeBody("NvmeState")), variants("NvmeState"));
  assert.deepEqual(literals(typeBody("AtaState")), variants("AtaState"));
  const fields = (src, re) => [...src.matchAll(re)].map((m) => m[1]).sort();
  assert.deepEqual(
    fields(
      types.match(/export interface AtaStats \{([\s\S]*?)\n\}/)[1],
      /^ {2}(\w+):/gm,
    ),
    fields(
      rust.match(/pub struct AtaStats \{([\s\S]*?)\n\}/)[1],
      /^ {4}pub (\w+):/gm,
    ),
  );
  assert.deepEqual(
    literals(typeBody("DiskOperational")),
    variants("Operational"),
  );
  assert.deepEqual(literals(typeBody("DiskHealthReport")), variants("Report"));
  assert.deepEqual(
    literals(typeBody("WindowsDiskStatus")).filter(
      (k) => !["healthy", "warning", "unhealthy", "unknown"].includes(k),
    ),
    variants("WindowsStatus"),
  );
});

test("sizes are decimal like the drive's units and counts never wrap", () => {
  assert.equal(formatDecimalBytes(0), "0 B");
  assert.equal(formatDecimalBytes(512_000), "512 KB");
  assert.equal(formatDecimalBytes(1_234_567_000_000), "1.2 TB");
  assert.equal(formatDecimalBytes(2.5e15), "2.5 PB");
  assert.match(formatDecimalBytes(1.7e44), /EB$/);
  assert.equal(formatCount(9876), "9,876");
  assert.equal(formatCount(3.4e38), "3.40e+38");
});

const log = (critical = 0) => ({
  critical_warning: {
    spare_low: !!(critical & 1),
    temperature: !!(critical & 2),
    reliability: !!(critical & 4),
    read_only: !!(critical & 8),
    backup_failed: !!(critical & 16),
    pmr_read_only: !!(critical & 32),
    raw: critical,
  },
});
const disk = (nvme, windows, ata = { kind: "not_asked" }) => ({
  nvme,
  windows,
  ata,
});

test("warnings come only from what the drive or Windows reported", () => {
  const healthy = {
    kind: "ok",
    health: "healthy",
    operational: [{ kind: "ok" }],
  };
  assert.deepEqual(
    driveWarnings(disk({ kind: "ok", log: log(0b101) }, healthy)),
    ["spare_low", "reliability"],
  );
  assert.equal(diskAlert(disk({ kind: "ok", log: log(0) }, healthy)), false);
  assert.equal(diskAlert(disk({ kind: "ok", log: log(2) }, healthy)), true);
  // Unknown or missing data is not a warning.
  assert.equal(
    diskAlert(disk({ kind: "denied" }, { kind: "not_reported" })),
    false,
  );
  assert.equal(
    diskAlert(
      disk(
        { kind: "not_asked" },
        { kind: "ok", health: "unknown", operational: [] },
      ),
    ),
    false,
  );
  assert.equal(
    diskAlert(
      disk(
        { kind: "not_asked" },
        {
          kind: "ok",
          health: "healthy",
          operational: [{ kind: "other", code: 53261 }],
        },
      ),
    ),
    false,
  );
  assert.equal(
    diskAlert(
      disk(
        { kind: "not_asked" },
        { kind: "ok", health: "warning", operational: [] },
      ),
    ),
    true,
  );
  assert.equal(operationalBad({ kind: "predictive_failure" }), true);
  assert.equal(operationalBad({ kind: "other", code: 1 }), false);
});

test("the watcher asks once on open, every 5 minutes, and never after stop", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  const replies = [{ kind: "ok", checked_at_ms: 1, disks: [] }];
  let calls = 0;
  const views = [];
  const stop = watchDiskHealth(
    async () => {
      calls++;
      return replies[0];
    },
    (view, pending) => views.push([view.kind, pending]),
  );
  await Promise.resolve();
  assert.equal(calls, 1, "one request when the view opens");
  t.mock.timers.tick(REFRESH_MS - 1);
  assert.equal(calls, 1, "no request before 5 minutes");
  t.mock.timers.tick(1);
  assert.equal(calls, 2, "a refresh at 5 minutes");
  stop();
  t.mock.timers.tick(REFRESH_MS * 10);
  assert.equal(calls, 2, "nothing after the view closes");
  assert.deepEqual(views[0], ["loading", false]);
});

test("a pending reply keeps the last report and retries once after 10 s", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  const ok = { kind: "ok", checked_at_ms: 1, disks: [] };
  const queue = [ok, { kind: "pending" }, ok];
  let calls = 0;
  const views = [];
  const stop = watchDiskHealth(
    async () => queue[calls++],
    (view, pending) => views.push([view.kind, pending]),
  );
  await Promise.resolve();
  await Promise.resolve();
  t.mock.timers.tick(REFRESH_MS);
  await Promise.resolve();
  await Promise.resolve();
  assert.deepEqual(
    views.at(-1),
    ["ok", true],
    "old report shown while pending",
  );
  t.mock.timers.tick(PENDING_RETRY_MS);
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(calls, 3);
  assert.deepEqual(views.at(-1), ["ok", false]);
  stop();
});

test("overlapping ticks never send a second request while one is out", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  let calls = 0;
  let resolve;
  const stop = watchDiskHealth(
    () => {
      calls++;
      return new Promise((r) => (resolve = r));
    },
    () => {},
  );
  t.mock.timers.tick(REFRESH_MS * 3);
  assert.equal(calls, 1);
  stop();
  resolve({ kind: "ok", checked_at_ms: 1, disks: [] });
});

test("a failed request is shown as failed, not as no disks", async () => {
  const views = [];
  const stop = watchDiskHealth(
    async () => {
      throw "WMI unavailable";
    },
    (view) => views.push(view),
  );
  await new Promise((r) => setTimeout(r, 0));
  stop();
  assert.deepEqual(views.at(-1), {
    kind: "failed",
    message: "WMI unavailable",
  });
});

const stats = (over = {}) => ({
  temperature_c: null,
  percent_used: null,
  power_on_hours: null,
  power_cycles: null,
  data_read_bytes: null,
  data_written_bytes: null,
  ...over,
});
const sata = (ata) =>
  disk({ kind: "not_asked" }, { kind: "not_reported" }, ata);

test("temperature and life used come from whichever log the drive answered", () => {
  assert.deepEqual(
    headline(
      disk(
        { kind: "ok", log: { ...log(0), temperature_c: 41, percent_used: 3 } },
        { kind: "not_reported" },
      ),
    ),
    { temperature_c: 41, percent_used: 3 },
  );
  assert.deepEqual(
    headline(sata({ kind: "ok", stats: stats({ temperature_c: 35 }) })),
    { temperature_c: 35, percent_used: null },
  );
  assert.deepEqual(
    headline(sata({ kind: "ok", stats: stats({ percent_used: 0 }) })),
    { temperature_c: null, percent_used: 0 },
    "0% is a reading, not missing",
  );
  for (const kind of ["sleeping", "denied", "unsupported", "bad_reply"])
    assert.deepEqual(headline(sata({ kind })), {
      temperature_c: null,
      percent_used: null,
    });
});

test("the note names the query the disk's bus was asked", () => {
  assert.deepEqual(queryNote(sata({ kind: "sleeping" })), {
    key: "dh.ata.sleeping",
  });
  assert.deepEqual(queryNote(sata({ kind: "error", code: 1117 })), {
    key: "dh.ata.error",
    vars: { v: 1117 },
  });
  assert.deepEqual(
    queryNote(sata({ kind: "ok", stats: stats({ temperature_c: 30 }) })),
    { key: "dh.ata.partial" },
  );
  const full = stats({
    temperature_c: 30,
    percent_used: 1,
    power_on_hours: 2,
    power_cycles: 3,
    data_read_bytes: 4,
    data_written_bytes: 5,
  });
  assert.equal(queryNote(sata({ kind: "ok", stats: full })), null);
  // USB and RAID keep the NVMe note; SATA no longer says "Windows status only".
  assert.deepEqual(queryNote(disk({ kind: "unsupported" }, {})), {
    key: "dh.nvme.unsupported",
  });
  assert.deepEqual(queryNote(disk({ kind: "error", code: 21 }, {})), {
    key: "dh.nvme.error",
    vars: { v: 21 },
  });
  assert.equal(queryNote(disk({ kind: "ok", log: log(0) }, {})), null);
  // ATA statistics never raise an alert by themselves.
  assert.equal(
    diskAlert(sata({ kind: "ok", stats: stats({ temperature_c: 70 }) })),
    false,
  );
});
