import assert from "node:assert/strict";
import { test } from "node:test";
import {
  helperView,
  createSensorSetupController,
} from "../src/lib/utils/sensor-setup.js";
import { setupState, setupSteps } from "../src/lib/utils/dashboard.js";
import { en, zh } from "../src/lib/i18n/strings.js";

const absent = {
  supported: true,
  bundled: true,
  pawnio_installed: false,
  installed: false,
  task: "missing",
  update_available: false,
  running: false,
  fake: false,
  installed_version: null,
  bundled_version: "0.8.2",
  busy: false,
};
const current = {
  ...absent,
  installed: true,
  pawnio_installed: true,
  task: "ok",
  running: true,
  installed_version: "0.8.2",
};
const older = {
  ...current,
  update_available: true,
  installed_version: "0.8.1",
};
function deferred() {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
}
function fixture(read = async () => absent, change = async () => current) {
  let state,
    reads = 0,
    changes = [];
  const controller = createSensorSetupController({
    read: () => {
      reads++;
      return read();
    },
    change: (enable) => {
      changes.push(enable);
      return change(enable);
    },
    publish: (v) => (state = v),
    delay: async () => {},
  });
  return {
    controller,
    get state() {
      return state;
    },
    get reads() {
      return reads;
    },
    changes,
  };
}

for (const [name, status, expected, enable, disable] of [
  ["absent", absent, "install", true, false],
  ["old", older, "update", true, true],
  [
    "mixed same version",
    { ...current, update_available: true },
    "repair",
    true,
    true,
  ],
  [
    "unknown version mismatch",
    { ...older, installed_version: null },
    "repair",
    true,
    true,
  ],
  [
    "newer installed version",
    { ...older, installed_version: "0.9.0" },
    "repair",
    true,
    true,
  ],
  ["current", current, "current", false, true],
  ["stopped", { ...current, running: false }, "stopped", true, true],
  ["missing task", { ...current, task: "missing" }, "repair", true, true],
  ["unknown task", { ...current, task: "unknown" }, "repair", true, true],
  [
    "missing driver",
    { ...current, pawnio_installed: false },
    "repair",
    true,
    true,
  ],
  [
    "bad source bundle",
    { ...older, bundled: false },
    "missing-files",
    false,
    true,
  ],
  [
    "valid installed without source",
    { ...current, bundled: false },
    "current",
    false,
    true,
  ],
  ["busy install", { ...absent, busy: true }, "install", false, false],
  ["unsupported", { ...absent, supported: false }, "unsupported", false, false],
  ["initial", null, "checking", false, false],
])
  test(`truthful helper view for ${name}`, () =>
    assert.deepEqual(helperView(status), { kind: expected, enable, disable }));

test("opening, refreshing, selecting consent and cancelling never install", async () => {
  const f = fixture();
  await f.controller.refresh();
  await f.controller.refresh();
  f.controller.request(true);
  assert.equal(f.state.activity.kind, "confirm");
  f.controller.dismiss();
  await f.controller.run();
  assert.deepEqual(f.changes, []);
  assert.equal(f.state.activity.kind, "idle");
});
test("Continue installs once, refreshes actual status, and blocks duplicate clicks", async () => {
  const pending = deferred();
  let installed = false;
  const f = fixture(
    async () => (installed ? current : absent),
    async () => {
      await pending.promise;
      installed = true;
      return current;
    },
  );
  await f.controller.refresh();
  f.controller.request(true);
  const running = f.controller.run();
  assert.equal(f.state.activity.kind, "working");
  f.controller.request(true);
  await f.controller.run();
  f.controller.dismiss();
  assert.deepEqual(f.changes, [true]);
  assert.equal(f.state.activity.kind, "working");
  pending.resolve();
  await running;
  assert.equal(f.state.status.installed_version, "0.8.2");
  assert.equal(f.state.feedback.kind, "success");
  assert.equal(f.state.activity.kind, "idle");
  f.controller.request(true);
  await f.controller.run();
  assert.deepEqual(f.changes, [true]);
});
test("UAC cancellation refreshes actual status, remains retryable and never retries elevation", async () => {
  const f = fixture(
    async () => absent,
    async () => {
      throw { kind: "cancelled" };
    },
  );
  await f.controller.refresh();
  f.controller.request(true);
  await f.controller.run();
  assert.equal(f.state.feedback.kind, "cancelled");
  assert.equal(f.state.status.installed, false);
  assert.deepEqual(f.changes, [true]);
  assert.equal(f.reads, 2);
  f.controller.request(true);
  assert.equal(f.state.activity.kind, "confirm");
});
test("copy failure refreshes partial install and offers repair", async () => {
  let failed = false;
  const f = fixture(
    async () =>
      failed ? { ...current, task: "missing", update_available: true } : absent,
    async () => {
      failed = true;
      throw { kind: "failed", message: "Cannot copy helper" };
    },
  );
  await f.controller.refresh();
  f.controller.request(true);
  await f.controller.run();
  assert.deepEqual(f.state.feedback, {
    kind: "error",
    message: "Cannot copy helper",
  });
  assert.equal(helperView(f.state.status).kind, "repair");
});
test("backend busy rejection is recoverable and prevents a second confirmation", async () => {
  let busy = false;
  const f = fixture(
    async () => ({ ...absent, busy }),
    async () => {
      busy = true;
      throw { kind: "busy" };
    },
  );
  await f.controller.refresh();
  f.controller.request(true);
  await f.controller.run();
  assert.deepEqual(f.state.feedback, { kind: "error", message: "busy" });
  f.controller.request(true);
  assert.equal(f.state.activity.kind, "idle");
  busy = false;
  await f.controller.refresh();
  f.controller.request(true);
  assert.equal(f.state.activity.kind, "confirm");
});
test("status failure is visible, drops stale status and can recover by read-only refresh", async () => {
  let fail = false;
  const f = fixture(async () => {
    if (fail) throw new Error("query denied");
    return current;
  });
  await f.controller.refresh();
  fail = true;
  await f.controller.refresh();
  assert.equal(f.state.status, null);
  assert.equal(
    helperView(f.state.status, f.state.feedback).kind,
    "unavailable",
  );
  assert.equal(f.state.feedback.kind, "error");
  fail = false;
  await f.controller.refresh();
  assert.equal(f.state.status.installed, true);
  assert.equal(f.state.feedback.kind, "none");
  assert.deepEqual(f.changes, []);
});
test("current helper cannot request reinstall, removal still requires Continue", async () => {
  let removed = false;
  const f = fixture(
    async () => (removed ? absent : current),
    async () => {
      removed = true;
      return absent;
    },
  );
  await f.controller.refresh();
  f.controller.request(true);
  await f.controller.run();
  assert.deepEqual(f.changes, []);
  f.controller.request(false);
  assert.deepEqual(f.changes, []);
  await f.controller.run();
  assert.deepEqual(f.changes, [false]);
  assert.equal(f.state.status.installed, false);
});
test("setup completed but helper not responding stays stopped after bounded refreshes", async () => {
  let installed = false;
  const stopped = { ...current, running: false };
  const f = fixture(
    async () => (installed ? stopped : absent),
    async () => {
      installed = true;
      return stopped;
    },
  );
  await f.controller.refresh();
  f.controller.request(true);
  await f.controller.run();
  assert.equal(f.reads, 10);
  assert.equal(helperView(f.state.status).kind, "stopped");
  assert.equal(f.state.activity.kind, "idle");
});
test("stale pre-install status never replaces post-install status", async () => {
  const old = deferred();
  let reads = 0;
  const f = fixture(() => {
    reads++;
    return reads === 2
      ? old.promise
      : Promise.resolve(reads > 2 ? current : absent);
  });
  await f.controller.refresh();
  const stale = f.controller.refresh();
  f.controller.request(true);
  const running = f.controller.run();
  old.resolve(absent);
  await stale;
  await running;
  assert.equal(f.state.status.installed, true);
  assert.equal(helperView(f.state.status).kind, "current");
});
test("concurrent status refreshes join one read and cannot start setup", async () => {
  const pending = deferred();
  const f = fixture(() => pending.promise);
  const a = f.controller.refresh(),
    b = f.controller.refresh();
  assert.equal(f.reads, 1);
  pending.resolve(absent);
  await Promise.all([a, b]);
  assert.deepEqual(f.changes, []);
});
test("first-launch and detail consumers use same status and preserve explicit driver consent steps", () => {
  assert.equal(setupState(absent), "enable");
  assert.equal(setupState(older), "update");
  assert.equal(setupState({ ...current, update_available: true }), "repair");
  assert.equal(setupState({ ...current, running: false }), "starting");
  assert.equal(setupState(current), "ready");
  assert.match(setupSteps(absent)[0], /PawnIO driver/);
  assert.equal(setupSteps(current).length, 2);
});
test("both locales retain meaningful helper scope, actions and outcome copy", () => {
  for (const table of [en, zh])
    for (const key of Object.keys(en).filter((k) => k.startsWith("helper."))) {
      assert.ok(table[key]);
      assert.doesNotMatch(table[key], /。/);
    }
  assert.match(en["helper.privacy"], /Windows user session/);
  assert.match(en["helper.privacy"], /whole fixed NTFS Disks/);
  assert.match(en["helper.privacy"], /folders you cannot open/);
  assert.match(en["helper.privacy"], /no file contents or writes/);
  assert.match(zh["helper.privacy"], /同一 Windows 用户会话/);
  assert.match(zh["helper.privacy"], /固定 NTFS 硬盘/);
  assert.match(zh["helper.privacy"], /无权打开/);
  assert.match(zh["helper.privacy"], /不读取文件内容，不写入文件/);
});

test("status loss closes stale consent without invoking setup", async () => {
  let fail = false;
  const f = fixture(async () => {
    if (fail) throw new Error("status unavailable");
    return absent;
  });
  await f.controller.refresh();
  f.controller.request(true);
  fail = true;
  await f.controller.refresh();
  assert.equal(f.state.activity.kind, "idle");
  assert.equal(f.state.feedback.kind, "error");
  await f.controller.run();
  assert.deepEqual(f.changes, []);
});
