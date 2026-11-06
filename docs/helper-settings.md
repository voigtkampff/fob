# Helper controls in Settings

## Goal and observable acceptance

Fob 0.8.2 keeps helper installation and updates discoverable in Settings after the initial prompt is dismissed. Settings › General › Sensor helper reuses the existing Temp & fan consent component. Opening Settings, refreshing status, and scanning cannot call elevated setup. Installation, replacement, repair, and removal still require explicit Continue followed by the existing Windows permission flow.

## Plan and grounding

- Ground. The existing SensorSetup component owns confirmation but previously kept local busy/error flags. The sensorSetup store queries status and invokes sensor_setup. Settings is a separate Tauri webview. Backend setup verifies bundled files, copies to protected Program Files, and re-registers the existing task. Sensor and MFT pipes stay unchanged.
- Sketch. Compare a new Settings-only installer with a shared lifecycle controller and component. Reject the separate installer because it duplicates consent and loses cross-entry busy/error state. Choose the shared component, a typed controller, full-manifest status checks, version-resource reads, and a backend-wide duplicate-operation gate.
- Agree. Proceed under the approved scope without a new checkpoint.
- Implement. Add the Settings card, shared controller, status/version data, locale copy, tests, and version bump. Keep installation mechanics and task security unchanged.
- Scrap. Revisit only if actual verification disproves the chosen shared ownership.

## Throughput checkpoint

- Blocking first steps. Verify live HEAD and clean status, inspect all setup consumers, and record typed contracts before implementation.
- Independent workstreams. No fan-out. The UI, store, backend status, and lifecycle tests form one coupled change. Independent test commands may run in parallel.
- Shared mutable state. Each webview has its own store. A backend atomic guard owns the real setup operation and exposes busy status to other views. Status refreshes never own or start setup.
- Smallest safe decomposition. One owner implements the coupled change and performs a separate adversarial review after automatic checks. No new workers.

## Contracts before logic

The backend status adds optional installed and bundled versions read from actual Windows product-version resources, plus busy status. Installed integrity compares every sensor/worker dependency against the manifest embedded in the running app. Bundle availability verifies both sensors and PawnIO manifest entries. A version is unknown if Windows cannot read it. Versions do not replace hash validation.

The pure helper view has unsupported, checking, unavailable, missing-files, install, update, repair, current, and stopped states. An older installed version with changed hashes offers Update. Equal or unknown versions with changed hashes offer Repair. Current files with no recent sensor response show Not responding rather than Running. Task query uncertainty never becomes a claim that the helper is healthy.

The frontend lifecycle contains the latest status, an idle/checking/confirm/working activity, and none/success/cancelled/error feedback. Status failures remove the stale status and permit retry. Setup errors preserve recoverable UI and refresh actual status. UAC cancellation is a typed cancelled result, not installation success. Pending status responses cannot overwrite post-operation state. Duplicate calls are rejected in both the controller and backend.

Cancellation before Continue only closes confirmation. Windows permission cancellation refreshes status without a retry or another UAC prompt. No new cancellation or forced termination is added to the existing elevated installer.

## Verification

Implementation and final evidence are recorded below as work completes. The user confirmed ordinary-launch helper MFT in v0.8.1. That report does not establish v0.8.2 Settings or consent behavior on Windows.

## Adversarial review

This owner performed a separate review of read-only refresh, stale replies, cross-window duplicate setup, stopped helpers, partial bundles, version-resource failure, and consent after status loss. The review found that a successful refresh retained a previous query error and that failed refresh could leave stale confirmation visible. Both cases now have reproducing behavior assertions and root-cause fixes in the controller. Status loss removes stale consent. Actual status recovery clears the old query error. Neither path invokes setup.

The safety fact is that refresh and confirmation selection cannot invoke the privileged operation. The controller tests observe zero setup calls for opening, repeated refresh, local cancellation, and stale consent. Only Continue calls change. A separate backend-wide atomic guard rejects concurrent setup across webviews and releases on all return paths. The Windows permission flow, file verification, protected deployment, task definition, driver consent, sensor pipe, MFT ACL, and fallback implementation are unchanged.

Installed/bundled product versions come from bounded Windows version-resource reads, not the Fob app version. Current status requires all installed sensor/worker hashes, the expected task, the driver registry entry, and a recent sensor response. No response is reported separately. An unreadable version stays Unknown. A corrupt or mixed source bundle cannot offer Install, Update, or Repair. Unknown task state never means a healthy helper.

Native Linux verification is currently blocked by absent pkg-config and GTK/WebKit development dependencies. Standard apt-get update against the configured official HTTP Debian repositories fails with HTTP 500. One transport-only attempt against the same official signed sources over HTTPS also fails with SSL unexpected EOF at the environment proxy. A standard package install cannot locate the required packages without indexes. No signature checks were disabled, no system repository configuration was changed, and no further retries were made. The exact Rust failure is in dist/v082-rust-tests.txt and the native dependency install output is in dist/v082-native-prerequisites.txt. Repair outbound access to the official Debian repositories, then install pkg-config, libwebkit2gtk-4.1-dev, libayatana-appindicator3-dev, librsvg2-dev and patchelf through apt-get before rerunning the full suite and current Linux app build. A cross-target compile is not counted as a full Rust test pass.

## Actual component preview

The isolated Vite fixture imports the actual SettingsWindow, SensorSetup, lifecycle controller, settings store, locales, styles and theme. Only Tauri transport replies are mocked. It does not enter the production route or packaged build and cannot install anything. The visible banner marks the mocked state and counts setup calls. The existing external Tauri mockIPC type declaration promises arbitrary generic results even for concrete mock responses; a scoped expected type error documents that test-transport declaration mismatch only. Production types remain checked.

Start with:

```bash
npx vite --config tests/fixtures/helper-settings.vite.config.js --host 0.0.0.0 --port 18082 --strictPort
```

Open http://127.0.0.1:18082/helper-settings.html?lang=zh&state=absent for Chinese, or change lang to en. State options are absent, old, mixed, current, stopped, busy, missing, and query-error. Outcome options success, cancel and error apply after explicit Continue. The error fixture supplies long path/detail text. Language can also change through the actual Settings select. Review at the Settings minimum 580 by 560 and normal 640 by 640. The fixture banner adds its own space outside the app component. No screenshot or interaction is claimed from HTTP transform checks alone. Parent desktop review is still needed, followed by Windows Settings/UAC runtime evidence. The user's v0.8.1 ordinary-launch MFT confirmation remains separate evidence.

## Settings commit verification

- `npm run check` reports zero errors and zero warnings.
- `node --test scripts/*.test.mjs` passes 138 tests, zero failures and zero skips, including 28 helper lifecycle tests and the eight unchanged doctor regressions.
- Every one of the ten `tests/*.mjs` scripts passes. Locale coverage includes 595 keys, essential consent disclosure, treemap and Other drilldown invariants, and existing Settings/mini behavior contracts.
- `npm run release:check`, `git diff --check`, and `bash -n scripts/package-windows.sh` pass. The project verification skill and doctor test file have no diff against f27fdc3.
- `cargo test --manifest-path sensors-helper/mft-worker/Cargo.toml` passes six tests. The actual helper build passes all 21 C# checks.
- A real x86_64-pc-windows-msvc package build compiles the changed backend and verifies all three executables as HCB 0.8.2 with asInvoker manifests and icons. This was an intermediate build before the last frontend copy/state correction. It is not the final delivery artifact and will be rebuilt after the approved lightweight integration.
- Full Linux `cargo test --manifest-path src-tauri/Cargo.toml` exits 101 in glib-sys because pkg-config is unavailable. Native prerequisite restoration is blocked as described above. The Linux full suite and current native startup/doctor are not passes.
- Isolated preview HTML, its transport fixture module and the real Settings component transform return HTTP 200. This is readiness evidence only, not a claim of browser interaction, screenshot review, or Windows layout acceptance.

Model the Domain led to one lifecycle controller instead of per-entry busy and error flags. Boundary Discipline keeps hash checks and version reads in the backend, and setup-error narrowing at the IPC boundary. Fix Root Causes removed stale lifecycle feedback/consent rather than changing the approved install flow. Prove It Works keeps mock, package, Linux dependency failure, and actual Windows evidence separate.
