import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  diskAlert,
  driveWarnings,
  formatCount,
  formatDecimalBytes,
  operationalBad,
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
const disk = (nvme, windows) => ({ nvme, windows });

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
