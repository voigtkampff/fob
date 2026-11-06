/** @typedef {import('../types').SensorSetupStatus} Status */
/** @typedef {'unsupported'|'checking'|'unavailable'|'missing-files'|'install'|'update'|'repair'|'current'|'stopped'} HelperKind */
/** @typedef {{kind: HelperKind, enable: boolean, disable: boolean}} HelperView */
/** @typedef {{kind:'idle'}|{kind:'checking'}|{kind:'confirm',enable:boolean}|{kind:'working',enable:boolean}} Activity */
/** @typedef {{kind:'none'}|{kind:'success',enable:boolean}|{kind:'cancelled'}|{kind:'error',message:string}} Feedback */
/** @typedef {{status:Status|null,activity:Activity,feedback:Feedback}} Lifecycle */

/** @param {Status|null|undefined} status @param {Feedback} [feedback] @returns {HelperView} */
export function helperView(status, feedback) {
  /** @type {HelperKind} */
  let kind;
  if (!status) kind = feedback?.kind === "error" ? "unavailable" : "checking";
  else if (!status.supported) kind = "unsupported";
  else if (
    status.installed &&
    !status.update_available &&
    status.task === "ok" &&
    status.pawnio_installed
  )
    kind = status.running ? "current" : "stopped";
  else if (!status.bundled) kind = "missing-files";
  else if (!status.installed && status.task === "missing") kind = "install";
  else if (
    status.installed &&
    status.update_available &&
    olderVersion(status.installed_version, status.bundled_version)
  )
    kind = "update";
  else kind = "repair";
  return {
    kind,
    enable:
      ["install", "update", "repair", "stopped"].includes(kind) &&
      !!status?.bundled &&
      !status?.busy,
    disable:
      !!status?.supported &&
      (status.installed || status.task !== "missing") &&
      !status.busy,
  };
}

/** @param {string|null|undefined} installed @param {string|null|undefined} bundled */
function olderVersion(installed, bundled) {
  if (
    !installed ||
    !bundled ||
    !/^\d+\.\d+\.\d+$/.test(installed) ||
    !/^\d+\.\d+\.\d+$/.test(bundled)
  )
    return false;
  const a = installed.split(".").map(Number),
    b = bundled.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] < b[i];
  return false;
}

/** @param {unknown} error @returns {Feedback} */
function failure(error) {
  if (typeof error === "object" && error !== null && "kind" in error) {
    if (error.kind === "cancelled") return { kind: "cancelled" };
    if (error.kind === "busy") return { kind: "error", message: "busy" };
    if ("message" in error && typeof error.message === "string")
      return { kind: "error", message: error.message };
  }
  return {
    kind: "error",
    message: error instanceof Error ? error.message : String(error),
  };
}

/**
 * @param {{read:()=>Promise<Status>,change:(enable:boolean)=>Promise<Status>,publish:(state:Lifecycle)=>void,delay:(ms:number)=>Promise<void>}} io
 */
export function createSensorSetupController(io) {
  /** @type {Lifecycle} */
  let state = {
    status: null,
    activity: { kind: "idle" },
    feedback: { kind: "none" },
  };
  /** @type {Promise<void>|null} */
  let reading = null;
  let revision = 0;
  /** @param {Partial<Lifecycle>} patch */
  function set(patch) {
    state = { ...state, ...patch };
    io.publish(state);
  }
  function refresh() {
    if (reading) return reading;
    const at = revision;
    if (!state.status && state.activity.kind === "idle")
      set({ activity: { kind: "checking" } });
    reading = io
      .read()
      .then((status) => {
        if (at === revision)
          set({
            feedback:
              !state.status && state.feedback.kind === "error"
                ? { kind: "none" }
                : state.feedback,
            status,
            activity:
              state.activity.kind === "checking"
                ? { kind: "idle" }
                : state.activity,
          });
      })
      .catch((error) => {
        if (at === revision)
          set({
            status: null,
            activity:
              state.activity.kind === "working"
                ? state.activity
                : { kind: "idle" },
            feedback: failure(error),
          });
      })
      .finally(() => {
        reading = null;
      });
    return reading;
  }
  /** @param {boolean} enable */
  function request(enable) {
    const view = helperView(state.status);
    if (
      state.activity.kind === "working" ||
      !(enable ? view.enable : view.disable)
    )
      return;
    set({ activity: { kind: "confirm", enable }, feedback: { kind: "none" } });
  }
  function dismiss() {
    if (state.activity.kind === "confirm")
      set({ activity: { kind: "idle" }, feedback: { kind: "none" } });
  }
  async function run() {
    if (state.activity.kind !== "confirm") return;
    const { enable } = state.activity;
    const view = helperView(state.status);
    if (!(enable ? view.enable : view.disable)) {
      dismiss();
      return;
    }
    revision++;
    set({ activity: { kind: "working", enable }, feedback: { kind: "none" } });
    /** @type {Feedback} */
    let feedback;
    try {
      const status = await io.change(enable);
      set({ status });
      feedback = { kind: "success", enable };
    } catch (error) {
      feedback = failure(error);
    }
    // The pre-operation read is invalidated, never joined as the final refresh.
    if (reading) await reading;
    await refresh();
    for (
      let i = 0;
      enable &&
      feedback.kind === "success" &&
      i < 8 &&
      state.status &&
      !state.status.running;
      i++
    ) {
      await io.delay(1500);
      await refresh();
    }
    set({
      activity: { kind: "idle" },
      feedback: state.status ? feedback : state.feedback,
    });
  }
  return { refresh, request, dismiss, run };
}
