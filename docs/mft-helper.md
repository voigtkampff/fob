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
- `node --test scripts/*.test.mjs` reports 98 passes and one existing failure. `scripts/mini-opacity.test.mjs` still expects a minimum of 40 while unchanged `src/lib/utils/mini-opacity.js` allows 0. Neither file nor the matching backend preference validation changed in this work.
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

The local test artifact is `dist/fob-v0.8.1-x86_64-windows.zip`, 8,283,961 bytes. Its SHA-256 is `281dd038823e4b91738b8e06e9232a7376a7505e361f7886395d64f9fa31efe2`.
