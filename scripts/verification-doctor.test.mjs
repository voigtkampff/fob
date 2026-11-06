import test from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const helper = fileURLToPath(
  new URL("../.cursor/skills/verify-fob/bin/fob-verify.sh", import.meta.url),
);

function doctor({
  pid = String(process.pid),
  mode = "window",
  logs = false,
} = {}) {
  const temp = mkdtempSync(join(tmpdir(), "fob-doctor-contract-"));
  try {
    const run = join(temp, "runs", "fixture");
    const bin = join(temp, "bin");
    const calls = join(temp, "calls.jsonl");
    mkdirSync(join(run, ".config", "com.hcb.fob"), { recursive: true });
    mkdirSync(bin);
    writeFileSync(join(run, ".config", "com.hcb.fob", "settings.json"), "{}");
    if (pid !== null) writeFileSync(join(run, "pid"), pid);
    if (logs) {
      const dir = join(run, ".local", "share", "com.hcb.fob", "logs");
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "fixture.txt"), "fixture");
    }
    writeFileSync(
      join(bin, "xdotool"),
      `#!/usr/bin/env node
import("node:fs").then(({ appendFileSync }) => {
  const args = process.argv.slice(2);
  appendFileSync(process.env.DOCTOR_CALLS, JSON.stringify(args) + "\\n");
  if (args[0] === "search") {
    const expected = ["search", "--onlyvisible", "--pid", process.env.DOCTOR_PID, "--name", "."];
    if (JSON.stringify(args) !== JSON.stringify(expected)) {
      console.log("1");
      process.exit(0);
    }
    if (process.env.DOCTOR_MODE === "empty") process.exit(0);
    if (process.env.DOCTOR_MODE === "failed-search") process.exit(1);
    console.log("101");
  } else if (args[0] === "getwindowname" && args[1] === "101") {
    console.log("Fob fixture");
  } else if (args[0] === "getwindowgeometry" && args[1] === "101") {
    console.log("Window 101\\n  Geometry: 1280x900");
  } else {
    console.log("root desktop");
  }
});
`,
      { mode: 0o755 },
    );
    const result = spawnSync("bash", [helper, "doctor", "fixture"], {
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${bin}:${process.env.PATH}`,
        FOB_VERIFY_RUNS: join(temp, "runs"),
        FOB_VERIFY_EVIDENCE: join(temp, "evidence"),
        DOCTOR_CALLS: calls,
        DOCTOR_PID: String(process.pid),
        DOCTOR_MODE: mode,
      },
      timeout: 5000,
    });
    assert.ifError(result.error);
    let observed = [];
    try {
      observed = readFileSync(calls, "utf8").trim().split("\n").map(JSON.parse);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    return { ...result, calls: observed };
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
}

test("doctor rejects a missing PID instead of accepting an unscoped root window", () => {
  const result = doctor({ pid: null });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /missing application pid/);
  assert.deepEqual(result.calls, []);
  assert.doesNotMatch(result.stdout, /alive pid|root desktop/);
});

test("doctor rejects PID zero before it can inspect the process group or desktop", () => {
  const result = doctor({ pid: "0" });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /invalid application pid/);
  assert.deepEqual(result.calls, []);
});

test("doctor rejects a malformed PID without searching for any window", () => {
  const result = doctor({ pid: "not-a-pid" });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /invalid application pid/);
  assert.deepEqual(result.calls, []);
});

test("doctor rejects an exited process even if a desktop window could be found", () => {
  const exited = spawnSync(process.execPath, ["-e", ""], { timeout: 5000 });
  assert.ifError(exited.error);
  assert.equal(exited.status, 0);
  assert.throws(() => process.kill(exited.pid, 0), { code: "ESRCH" });
  const result = doctor({ pid: String(exited.pid) });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /application is not running/);
  assert.deepEqual(result.calls, []);
});

for (const mode of ["empty", "failed-search"]) {
  test(`doctor rejects a live process without its own visible window (${mode})`, () => {
    const result = doctor({ mode });
    assert.equal(result.status, 1);
    assert.match(result.stdout, /no visible application window/);
    assert.deepEqual(result.calls, [
      ["search", "--onlyvisible", "--pid", String(process.pid), "--name", "."],
    ]);
    assert.doesNotMatch(result.stdout, /alive pid|root desktop/);
  });
}

for (const logs of [false, true]) {
  test(`doctor accepts its live process and scoped window with logs ${logs ? "present" : "absent"}`, () => {
    const result = doctor({ logs });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, new RegExp(`alive pid ${process.pid}`));
    assert.match(result.stdout, /window 101 'Fob fixture'/);
    assert.match(result.stdout, /Geometry: 1280x900/);
    assert.match(result.stdout, /config: settings.json/);
    assert.deepEqual(result.calls[0], [
      "search",
      "--onlyvisible",
      "--pid",
      String(process.pid),
      "--name",
      ".",
    ]);
    assert.doesNotMatch(result.stdout, /root desktop/);
    if (logs) assert.match(result.stdout, /log: fixture.txt/);
    else
      assert.match(result.stdout, /no application log directory \(optional\)/);
  });
}
