import test from "node:test";
import assert from "node:assert/strict";
import {
  existsSync,
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
const display = process.env.DISPLAY_FOB || process.env.DISPLAY || ":55";

// The bash this file spawns, resolved once. The capability test below runs the
// doctor with a PATH that holds no xdotool at all, so it cannot rely on PATH
// lookup for the shell itself.
const bash = (() => {
  for (const candidate of [process.env.SHELL, process.env.BASH]) {
    if (candidate && existsSync(candidate)) return candidate;
  }
  const found = spawnSync("bash", ["-c", "command -v bash"], {
    encoding: "utf8",
  });
  let path = (found.stdout || "").trim();
  if (path && !existsSync(path) && process.platform === "win32") {
    // Git Bash reports /usr/bin/bash; Windows needs the real path to spawn it
    // once the child PATH no longer contains it.
    const win = spawnSync("cygpath", ["-w", path], { encoding: "utf8" });
    if (win.status === 0) path = (win.stdout || "").trim();
  }
  return path && existsSync(path) ? path : "bash";
})();

// Capability, not the platform name: the doctor drives a real desktop through
// xdotool, so it can only be held to its window contract where an X server
// answers. A Linux box without xdotool skips for the same reason Windows does
// instead of going green on a lie.
function missingX11() {
  const probe = spawnSync("xdotool", ["getdisplaygeometry"], {
    encoding: "utf8",
    env: { ...process.env, DISPLAY: display },
    timeout: 5000,
  });
  if (probe.error) {
    return `needs X11 and xdotool: xdotool could not run (${probe.error.code ?? probe.error.message})`;
  }
  if (probe.status !== 0) {
    return `needs X11 and xdotool: no X server answered on DISPLAY=${display}`;
  }
  return false;
}

const skipWithoutX11 = missingX11();

const shim = () => `#!/usr/bin/env node
import("node:fs").then(({ appendFileSync }) => {
  const args = process.argv.slice(2);
  appendFileSync(process.env.DOCTOR_CALLS, JSON.stringify(args) + "\\n");
  if (args[0] === "getdisplaygeometry") {
    if (process.env.DOCTOR_PROBE === "unreachable") {
      console.error("Error: Can't open display: " + process.env.DISPLAY);
      process.exit(1);
    }
    console.log("1600 1000");
  } else if (args[0] === "search") {
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
`;

function doctor({
  pid = String(process.pid),
  mode = "window",
  logs = false,
  probe = "ok",
  xdotool = true,
} = {}) {
  const temp = mkdtempSync(join(tmpdir(), "fob-doctor-contract-"));
  try {
    const run = join(temp, "runs", "fixture");
    const bin = join(temp, "bin");
    const empty = join(temp, "empty");
    const calls = join(temp, "calls.jsonl");
    mkdirSync(join(run, ".config", "com.hcb.fob"), { recursive: true });
    mkdirSync(bin);
    mkdirSync(empty);
    writeFileSync(join(run, ".config", "com.hcb.fob", "settings.json"), "{}");
    if (pid !== null) writeFileSync(join(run, "pid"), pid);
    if (logs) {
      const dir = join(run, ".local", "share", "com.hcb.fob", "logs");
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "fixture.txt"), "fixture");
    }
    if (xdotool) writeFileSync(join(bin, "xdotool"), shim(), { mode: 0o755 });
    const result = spawnSync(bash, [helper, "doctor", "fixture"], {
      encoding: "utf8",
      env: {
        ...process.env,
        // Without xdotool the whole PATH is emptied: the doctor has to give up
        // on the platform before it reaches for anything else.
        PATH: xdotool ? `${bin}:${process.env.PATH}` : empty,
        DISPLAY: display,
        FOB_VERIFY_RUNS: join(temp, "runs"),
        FOB_VERIFY_EVIDENCE: join(temp, "evidence"),
        DOCTOR_CALLS: calls,
        DOCTOR_PID: String(process.pid),
        DOCTOR_MODE: mode,
        DOCTOR_PROBE: probe,
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
    return {
      ...result,
      calls: observed,
      searches: observed.filter((args) => args[0] === "search"),
    };
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
}

const unsupported =
  /unsupported on this platform: the verification harness needs X11 and xdotool/;

test("doctor says the platform is unsupported instead of inventing a dead app when xdotool is missing", () => {
  const result = doctor({ xdotool: false });
  assert.equal(result.status, 3);
  assert.match(result.stdout, unsupported);
  assert.match(result.stdout, /xdotool is not installed/);
  assert.doesNotMatch(result.stdout, /application is not running/);
  assert.deepEqual(result.calls, []);
});

test("doctor says the platform is unsupported when no X server answers on DISPLAY", () => {
  const result = doctor({ probe: "unreachable" });
  assert.equal(result.status, 3);
  assert.match(result.stdout, unsupported);
  assert.match(result.stdout, /no X server answers on DISPLAY/);
  assert.doesNotMatch(result.stdout, /application is not running/);
  assert.deepEqual(result.calls, [["getdisplaygeometry"]]);
  assert.deepEqual(result.searches, []);
});

test("doctor rejects a missing PID instead of accepting an unscoped root window", () => {
  const result = doctor({ pid: null });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /missing application pid/);
  assert.deepEqual(result.searches, []);
  assert.doesNotMatch(result.stdout, /alive pid|root desktop/);
});

test("doctor rejects PID zero before it can inspect the process group or desktop", () => {
  const result = doctor({ pid: "0" });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /invalid application pid/);
  assert.deepEqual(result.searches, []);
});

test("doctor rejects a malformed PID without searching for any window", () => {
  const result = doctor({ pid: "not-a-pid" });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /invalid application pid/);
  assert.deepEqual(result.searches, []);
});

test("doctor rejects an exited process even if a desktop window could be found", () => {
  const exited = spawnSync(process.execPath, ["-e", ""], { timeout: 5000 });
  assert.ifError(exited.error);
  assert.equal(exited.status, 0);
  assert.throws(() => process.kill(exited.pid, 0), { code: "ESRCH" });
  const result = doctor({ pid: String(exited.pid) });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /application is not running/);
  assert.deepEqual(result.searches, []);
});

for (const mode of ["empty", "failed-search"]) {
  test(
    `doctor rejects a live process without its own visible window (${mode})`,
    { skip: skipWithoutX11 },
    () => {
      const result = doctor({ mode });
      assert.equal(result.status, 1);
      assert.match(result.stdout, /no visible application window/);
      assert.deepEqual(result.searches, [
        [
          "search",
          "--onlyvisible",
          "--pid",
          String(process.pid),
          "--name",
          ".",
        ],
      ]);
      assert.doesNotMatch(result.stdout, /alive pid|root desktop/);
    },
  );
}

for (const logs of [false, true]) {
  test(
    `doctor accepts its live process and scoped window with logs ${logs ? "present" : "absent"}`,
    { skip: skipWithoutX11 },
    () => {
      const result = doctor({ logs });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, new RegExp(`alive pid ${process.pid}`));
      assert.match(result.stdout, /window 101 'Fob fixture'/);
      assert.match(result.stdout, /Geometry: 1280x900/);
      assert.match(result.stdout, /config: settings.json/);
      assert.deepEqual(result.searches[0], [
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
        assert.match(
          result.stdout,
          /no application log directory \(optional\)/,
        );
    },
  );
}
