# Elevated MFT scans

## Goal

A normally launched Fob uses the installed elevated sensor helper for a read-only NTFS scan. The main executable stays asInvoker. Scanning never requests elevation.

## Architecture and alternatives

The sensor JSON stream remains unchanged. A separate current-user named pipe owns one scan at a time. The helper launches a bundled, hash-pinned Rust worker from its protected installation directory. The worker shares the NTFS reader with the direct scan. The UI builds the existing folded Node tree and cache.

Putting scan traffic on the sensor pipe would block sensor updates and break old helpers. Implementing another NTFS parser in C# would duplicate the size, hard-link, and reparse-point rules. USN enumeration alone does not supply equivalent sizes and hard-link names. The small Rust worker avoids these alternatives.

## Contract before implementation

Request is exactly eight bytes, `FM01`, one uppercase ASCII drive letter, and three zero bytes. It authorizes only the root of that fixed local NTFS drive. No subtree, UNC, device path, filename, or command string is accepted.

Response starts with `FM01`. Frames contain a one-byte kind and a four-byte little-endian length, followed by at most 4096 bytes. Kind 1 contains one JSON MftLink. Kind 2 is an empty successful end. Kind 3 contains a bounded error code. Kind 4 is an empty heartbeat. An EOF without a successful end is an error, never a partial result.

Cancellation is pipe closure or any byte after the request. The helper closes the pipe and kills its own worker. The client checks cancellation while polling at 20 ms intervals. A two-second request deadline, fifteen-second client idle deadline, and three-minute total deadline bound waits. Accumulation is bounded by record count and name bytes, with no queued copies of the result.

## Work checklist

- [x] Ground the existing helper setup, sensor stream, and space cache
- [x] Sketch two alternatives and select the shared Rust worker
- [x] Agree without another user checkpoint, the integration is approved
- [x] Implement the bounded request, worker, client, and fallback
- [x] Keep the design after the framed fixture passes
- [x] Run behavior tests, adversarial review, local cross-build, and package checks

The throughput checkpoint is completion of one real framed scan fixture through the shared decoder into the existing tree. Only then run full builds and package the local artifact.

## Deployment and security boundary

The existing explicit Update sensor helper flow stops the scheduled task, copies the manifest-verified bundle to `%ProgramFiles%\Fob\sensors`, verifies the destination, registers the same current-user elevated logon task, and starts it. Existing installs need that one-time confirmation and UAC prompt. There is no automatic installation, driver change, or permission prompt during a scan. Main Fob remains asInvoker. A missing, old, mixed, busy, disconnected, or timed-out helper falls back to direct MFT when permitted, then to the unchanged SSD-aware walk.

The pipe name includes the elevated helper's user SID. Its protected DACL grants only that SID data and attribute access, not CreateNewInstance, and denies network logons. A medium integrity mandatory label admits the ordinary UI but denies low-integrity writes. The native server rejects remote clients and reserves the first instance. After reading the bounded request, it checks the client's impersonated SID and process session before querying a drive or starting the worker. SUBST paths, network mappings, and directory aliases are rejected by the volume-device check.

The UI authenticates the server PID, installed executable path, same user and session, elevation token, and exact bundled manifest hashes. A same-user pipe squatter cannot impersonate an elevated matching helper; it can deny service, in which case the UI walks instead. The helper holds the hash-checked worker executable open without write or delete sharing while it runs. The worker receives only one drive letter. Helper termination closes the worker's stdin and ends the worker, including during an update.

This assumes the normal administrator-protected Program Files ancestry and installation ACLs, an uncompromised administrator, and a trusted Fob executable. It does not defend against an administrator replacing loaded code. Any medium-integrity process under the same account and session can request metadata for a fixed NTFS volume. It cannot request file contents, arbitrary paths, process commands, or writes. Alternate-account UAC credentials do not grant the original account access; the UI falls back instead.

Only whole-volume requests exist. Live Other listings still use the ordinary UI's existing read permissions and do not gain elevated arbitrary-folder access. MFT allocation sizes retain the 0.8.0 cluster-rounded logical-size rules, including their limitations for compressed, sparse, and resident files.

## Review and verification

A distinct adversarial pass checked the native access masks, mandatory integrity label, impersonation ordering, process identity, SUBST aliases, missing stream terminators, overflowing totals, cancellation, worker lifetime, and installation update path. It caught and corrected the missing medium integrity label and the attempt to impersonate before reading a request. Directory cycles, duplicate or conflicting identities, oversized names and frames, total size overflow, and partial streams are rejected rather than cached.

Model the Domain led to a single drive-root request and a bounded metadata record stream. Boundary Discipline kept authorization and framing checks at IPC. Experience First kept scans free of UAC prompts and preserved fallback. Test Behavior, Not Implementation led to framed tree and actual polling-reader deadline tests. Prove It Works led to checks of the shipped PE resources and bundle hashes instead of build timestamps.

Executed on Linux:

- `cargo test --manifest-path src-tauri/Cargo.toml` passes 129 tests, with two existing opt-in benchmarks ignored.
- `cargo test --manifest-path sensors-helper/mft-worker/Cargo.toml` passes six protocol tests.
- `/workspace/tools/dotnet/dotnet run --project sensors-helper/tests -c Release` passes 21 checks, including the unchanged sensor fixture and the new request validation.
- `npm run check` reports zero Svelte errors and warnings. `npm run release:check` reports 0.8.1.
- The initial `node --test scripts/*.test.mjs` run reported 98 passes and one stale opacity test failure. The review follow-up corrects the test contract and the full suite now passes all 102 tests. Production opacity behavior and backend validation are unchanged.
- All ten `tests/*.mjs` scripts pass, including copy, both locales, treemap, Other, navigation, and dashboard tests.
- The existing verify-fob build and launch procedure starts a real Linux app. The doctor confirms a live process and a visible 1280 by 900 Fob window. Its final exit code is 2 because it lists a log directory that no longer exists, not because the app failed. A Linux startup screenshot is retained. The changed setup confirmation and space-result method labels have not been driven in that UI.

`scripts/package-windows.sh` builds the actual .NET 4.7.2 helper, the Rust worker, the frontend, and `cargo xwin build --target x86_64-pc-windows-msvc --release`. Its resource checks require HCB, version 0.8.1, the asInvoker manifest, and icons in all three executables. The final package also retains the existing com.hcb.fob identifier and includes the MFT reader's MIT notice.

Not verified on Windows: medium-to-high integrity pipe communication, the one-time update and restart, live NTFS results and timings, sensor updates during a scan, cancellation of real disk I/O, remote or low-integrity denial, and the changed visible copy. No new speed result is claimed. The user's earlier 24-second versus 4.2-second measurement is not a measurement of this integration.

## Acceptance evidence and delivery gate

This is a pending-verification Windows test package, not Windows acceptance.

| Observable acceptance                                     | Evidence available                                                                                                                                     | Still missing                                                  |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------- |
| Ordinary asInvoker launch obtains helper MFT              | Compiled authenticated client, actual helper and worker builds, manifest and worker-pin audit                                                          | Windows ordinary-launch request and resulting MFT label        |
| Missing, old, or incompatible helper falls back           | Missing-helper scan test returns the real walk and its method; fixture version and rejection tests; installed-file hash checks                         | Windows old-helper and mixed-install paths                     |
| Cancel, disconnect, and timeout do not cache partial data | Framed EOF and error tests; polling-reader idle, total, and cancellation tests; successful sentinel required                                           | Windows pipe closure during actual disk I/O                    |
| Same tree, hard links, and Other                          | Existing MFT fixture, framed hard-link tree, full Rust walk/cache/Other tests, JS treemap and navigation tests                                         | Windows visual drilldown of a helper-created result            |
| Sensors and UI remain responsive during scans             | Separate pipe, worker, and accept thread; unchanged sensor fixture and sampler tests                                                                   | Concurrent Windows sensor stream and scan observation          |
| Main stays asInvoker                                      | Actual packaged PE manifest check                                                                                                                      | Windows process-token observation                              |
| Existing explicit upgrade works                           | Existing verified setup and task path retained, updated helper detected by changed executable, package includes worker                                 | One-time Windows update, restart, and UAC observation          |
| Package components match                                  | All three PE resources pass HCB 0.8.1 checks; zipped binaries equal build outputs; embedded manifest and worker SHA pin match every bundled executable | Target-platform launch of this exact zip                       |
| Affected copy and interactions fit                        | Both locale tables, 1160-string copy check, and Svelte check pass                                                                                      | Actual affected setup dialog and scan-result labels on Windows |

No Windows machine was operated, VM installed, or external upload made. The feasible automated checks ran first. Remaining target-platform evidence is a delivery gate, not a request for the user to perform full QA.

The initial artifact before the later permission-copy review was `dist/fob-v0.8.1-x86_64-windows.zip`, 8,283,961 bytes. Its SHA-256 was `281dd038823e4b91738b8e06e9232a7376a7505e361f7886395d64f9fa31efe2`.

## Verification-only follow-up 72ff370 on 979f5b4

The failing opacity test was reproduced before editing. Its old expectation `miniOpacity(12) === 40` was the fault, not the utility. The actual contract is consistent across the utility, backend, and slider:

- `src/lib/utils/mini-opacity.js` exports minimum 0, maximum 100, and step 1. It preserves every whole percent, accepts numeric strings, rounds finite fractional values, clamps out-of-range values, and returns 100 for invalid or non-finite inputs.
- `src/lib/components/OpacitySlider.svelte` uses those bounds and the one-percent step. Mini opacity is not restricted to a few preset levels. The shared status-bar use can supply its own minimum and is unchanged.
- `src-tauri/src/preferences.rs` accepts saved integer miniOpacity values from 0 through 100. Invalid raw saved values are rejected, leaving the previous setting intact. The adapter's coercion and the backend's rejection are intentionally different boundaries.

Only `scripts/mini-opacity.test.mjs` and this document changed in the verification-only commit 72ff370. The revised tests exercise every integer from 0 through 100 and its numeric-string equivalent, negative and over-100 clamping, half-percent rounding, the former low-range failure, invalid and non-finite defaults, explicit UI bounds, the one-percent step, backend range agreement, shared slider use, opaque text, and transparent mini-window configuration. No production behavior, layout, visible strings, or skill files change.

The follow-up commands and results are:

- `node --test scripts/mini-opacity.test.mjs` initially reproduces the old failure, one pass and one failure.
- `node --test scripts/*.test.mjs` after correction passes 102 tests, zero failures and zero skips.
- `npm run check` reports zero errors and zero warnings.
- `for f in tests/*.mjs; do node "$f" || exit; done` passes all ten scripts, including opacity contrast at 0/25/50/75/100 across 24 themes, copy, locales, treemap, and Other regression coverage.
- `cargo test --manifest-path src-tauri/Cargo.toml` passes 129 tests, with the same two opt-in benchmarks ignored. This includes `preferences::tests::mini_opacity_is_opaque_by_default_and_kept_in_range`.
- `cargo test --manifest-path sensors-helper/mft-worker/Cargo.toml` passes six tests.
- `/workspace/tools/dotnet/dotnet run --project sensors-helper/tests -c Release` passes 21 checks.

The doctor defect is in the skill-owned file `/workspace/winperf/.cursor/skills/verify-fob/bin/fob-verify.sh`, line 42. Its unconditional `ls "$LOGS"` runs under `set -euo pipefail`. The deliberately absent application log directory therefore makes doctor exit 2 after it has already printed a live process and visible window. The script was not edited under the explicit no-skill-edits constraint. The needed skill-owner fix is to make listing that optional directory conditional, report that absent logs are expected, and retain explicit failing checks for a dead process or missing visible app window. Creating a log directory or restoring log writers would hide the tool defect and is not a fix.

Valid Linux smoke was rerun separately from the doctor verdict:

1. `BIN=/workspace/scratch/verify/winperf-mft081 .cursor/skills/verify-fob/bin/fob-verify.sh relaunch mft081 WINPERF_SPACE_ROOTS=/workspace/winperf/tests` starts the existing Linux verification binary.
2. Independent assertions read its actual PID, use `os.kill(pid, 0)` to prove it is alive, require a nonempty `xdotool search --onlyvisible --pid PID --name Fob` result on display :55, inspect the title and geometry, and assert the removed application log directory is absent.
3. Observed process 875102 has visible window 2097155, title Fob, geometry 1280 by 900. Those assertions pass. The unmodified doctor then reproduces exit 2 on the same live process, isolating the verification-tool defect from application status.
4. The existing stop command terminates only that verification process. No log writers or application log directories were added. This is startup/liveness evidence, not validation of the changed MFT dialogs or Windows execution.

The existing zip was not rebuilt or repackaged for test and documentation changes. A fresh audit compares all three packaged executable byte streams against their compiled outputs, verifies every entry in the embedded helper manifest and program SHA256SUMS, confirms the helper's UTF-16 worker SHA pin, and reads the actual PE resources. Main, helper, and worker still pass HCB 0.8.1, icon, and asInvoker checks. `git diff --quiet 979f5b4 -- src src-tauri sensors-helper/src sensors-helper/mft-worker sensors-helper/build.sh scripts/package-windows.sh` confirms production source and packaging are unchanged. The zip remains 8,283,961 bytes with SHA-256 `281dd038823e4b91738b8e06e9232a7376a7505e361f7886395d64f9fa31efe2`.

Fix Root Causes kept the approved 0–100 behavior and repaired the obsolete test instead. Prove It Works required a real live-process/window check and actual zip inspection, rather than treating the doctor's unrelated exit status or a prior build as proof. The artifact remains a pending-verification Windows test package. No Windows acceptance is claimed and no full QA assignment is handed to the user.

## Independent privilege review and upgrade trace

The independent read-only review reported no source blocker in the IPC access mask, medium integrity label, request-before-impersonation ordering, server identity and hashes, or worker lifetime. No weaker executable-path-only client gate was introduced.

The permission confirmation now plainly discloses that the helper can list names and sizes across entire NTFS Disks, including folders the user cannot normally open. Both Chinese and English explain that Fob keeps ordinary permissions, no file contents are returned, and fan speeds are not changed. `SensorSetup.svelte:44–58` shows this note before Continue for both initial setup and updates. `tests/i18n.mjs` checks the necessary scope and no-contents disclosures in both locales. The note includes other-user and administrator-only folders through the explicit normally-inaccessible-folder wording. This is a consequence of whole-volume MFT access, not a new capability in this follow-up. Actual target-platform layout and interaction remain unverified.

The helper remains single-instance. An unavailable or busy pipe fails the open at `space_helper.rs:176–187`. `space.rs:459–475` then tries direct MFT if permitted or uses the existing walk. A successful helper result is labeled MFT via helper or MFT 助手扫描. A direct scan is labeled MFT. Walk fallback is labeled Folder scan or 文件夹扫描 by `SpacePage.svelte:291–298` and the locale tables. No wait queue or pipe-wait feature was added.

The actual update sequence was traced through these source boundaries:

1. `sensors/setup.rs:384–388` invokes `task::stop`, waits 1500 ms, and stops the old ETW session. `sensors/task.rs:89–92` calls the Task Scheduler COM task's `Stop(0)`. The existing task XML explicitly permits hard termination at `setup.rs:164`.
2. During a live scan, `MftService.cs:105–106` holds the verified worker executable with `FileShare.Read`, which prevents write or delete replacement. Normal completion or cancellation closes worker stdin and kills/disposes the worker in `MftService.cs:140–147`.
3. If task termination kills the helper instead of running its finally block, Windows closes the helper's file handles and the redirected parent-side stdin handle. The worker's stdin monitor at `mft-worker/src/main.rs:17–22` exits the worker when that stream closes. Its executable mapping is released when the worker exits. This is a source-level lifecycle trace, not a Windows lock-release observation.
4. `setup.rs:422–440` stops the task before removing or copying the destination. It retries the removal/copy up to three times with one-second gaps. It only accepts a copy when every requested file copied successfully and the complete destination matches the embedded manifest. A surviving executable lock, incomplete copy, missing worker, or mismatched worker cannot pass that check.
5. The failure path returns exit code 12. `run_elevated` maps the nonzero exit to the existing visible "Couldn't copy the sensor helper to Program Files" error. `SensorSetup.svelte:24–34` retains the confirmation and shows that error through its role=alert paragraph. Task registration and `run_task()` at `setup.rs:449–450` are reached only after full verification succeeds.
6. A failed update is not an atomic rollback. Partial files can remain on disk after an error, and the old task registration can remain. The copy failure is reported rather than silently declared successful. Ordinary scanning separately requires the exact full helper manifest and falls back on a mixed installation instead of labeling it MFT. Windows termination timing, executable mapping release, and retry behavior under an actual file lock still require Windows evidence.

A new behavior test, `sensors::setup::tests::mixed_or_incomplete_helper_worker_bundle_is_rejected`, starts with a matching helper/worker pair, checks success, replaces the worker with an old version and checks rejection, then removes it and checks rejection. It also checks the existing copy-error mapping. The targeted setup suite passes five tests. No production update logic needed modification based on this trace.

Because the permission text is shipped inside Fob, this review changes a production frontend asset. The Windows test artifact is rebuilt for that change. The earlier verification-only phase did not rebuild or repackage identical binaries. The skill-owned doctor defect remains reported and unedited; no new application log writers or layout changes were made.

### Current review artifact and final automatic checks

After the permission-scope change, `node --test scripts/*.test.mjs` passes all 102 tests. `npm run check` reports zero errors and warnings. All ten `tests/*.mjs` scripts pass, including the new disclosure checks. Full Rust tests pass 130 tests with two opt-in benchmarks ignored. The targeted setup suite passes five tests. `cargo test --manifest-path src-tauri/Cargo.toml --offline mft` passes eleven tests. Worker protocol tests remain six passes, and the actual helper build reruns all 21 C# checks successfully.

`PATH=/workspace/tools/xbin:/workspace/tools/llvm-mingw-20261020-ucrt-ubuntu-22.04-x86_64/bin:$PATH scripts/package-windows.sh` rebuilds the changed frontend and x86_64-pc-windows-msvc application. All three actual packaged executables pass the HCB 0.8.1, asInvoker manifest, and icon resource checks. A fresh zip audit confirms that each executable equals its compiled output, the embedded helper manifest matches every bundled program, all program SHA256SUMS match, and the helper's worker pin matches the worker. Both permission-scope strings are present in the newly built frontend assets.

The current artifact supersedes the earlier 281dd038 checksum recorded above:

- Zip: `/workspace/winperf/dist/fob-v0.8.1-x86_64-windows.zip`
- Size: 8,284,112 bytes
- SHA-256: `ec6dc9a7cb497c7b54d020bb75beb3e74add989cfb4758fa9d458deed536b5a4`
- Checksum sidecar: `/workspace/winperf/dist/fob-v0.8.1-x86_64-windows.zip.sha256`

This remains a pending-verification Windows test package. Ordinary-launch MFT, sensor responsiveness during real scans, actual upgrade lock release, Windows access denial and cancellation, and the affected permission-note layout are not Windows-verified. The independent source review and automatic tests do not replace those observations.
