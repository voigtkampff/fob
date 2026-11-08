# Same-source process-cache comparison

## Comparison contract, before modifications

The user wants a control that can isolate the 0.8.2 process-metadata cache change.
This task prepares and verifies test packages on the box. It does not launch either
variant on Windows, stop the user's running Fob, or claim a performance benefit.

- B is the production application source pinned at e9672f1. Its src, src-tauri,
  sensors-helper and package inputs equal dce9148. Later changes are verification,
  documentation and the standalone sampler.
- A starts at the same pin and restores only the production code in
  src-tauri/src/monitoring/process_monitor.rs and types.rs from ce2adfc's parent,
  e1a6e17. ce2adfc and 4ffef1d are the optimization and follow-up test/owner commits.
  No other application file is reverted. Test-only additions can characterize both
  variants without affecting release code.
- Both remain version 0.8.2, HCB, com.hcb.fob and asInvoker. No UI, settings schema,
  helper protocol, helper bundle, permissions, PawnIO copy or roadmap work is changed.
  Variant names live in outer artifact names and comparison manifests.
- Original production archive SHA256 must remain
  0cab43bcd4c59abb6d932f95f908b53181342bfcb4eb07c52c975ea082ed317f.
  Sampler archive SHA256 must remain
  d60f460856ccfa28f41d62061870c9f95a87abb3d45c9fe1acf0b3be5307b8fd.
- Build fresh paired A and B in isolated local worktrees and separate target
  directories. The old B is retained as reference, not assumed to match newly
  generated frontend or build inputs. Both new variants receive identical frontend
  assets and the exact helper payload from the original archive, not rebuilt helpers.
- Record toolchain, flags, source and generated-input hashes. Compare the new B to
  the user's original executable rather than assuming byte equivalence.

## How the changed code works

Hub capture collects sysinfo process readings, then ProcessMonitor converts them to
ProcessData and serializable ProcessInfo. CPU normalization, dynamic counters and
returned timestamps are independent of metadata storage. The optimized B stores
static details by pid plus start time, retries missing metadata, shares environment
arrays and prunes exited entries. The old A copies command/environment each
collection, keeps a pid-only static cache and does not prune it.

The rollback intentionally restores old lifecycle limitations. A can retain an old
name, command or owner after PID reuse, retain dead entries internally, and fail to
fill a command missing at first observation. B avoids those cases. Ordinary stable
process JSON, dynamic counters, status and timestamps should match. This experiment
therefore measures the whole metadata-cache change, including its correctness and
cleanup changes, not just Arc allocation. A is comparison-only, not a replacement
release. Do not use process actions while evaluating the control.

## Execution checklist

1. Capture a baseline trace via the matching control skill. Vet the baseline, and each later number, with the benchmark-checklist skill.
   Existing short Windows B pilot validates the sampler only. No matched baseline
   trace is claimed or run during this preparation.
2. how to ground hypotheses. Don't claim a perf ceiling without running it first.
   The flow and known old/new lifecycle differences are described above.
3. Plan the fix from the trace. If it crosses a function boundary, architect first. Delegate implementation to a subagent using your configured perf-issue model. Review the diff. Capture a post-fix trace.
   Narrow exception here is baseline preparation, not another performance fix.
   No subagent tool is available. Local-only work excludes cloud checkout.
4. Parse and compare the artifacts. Inconclusive or wrong-surface is not a pass.
   Compare source boundaries, behavior tests, inputs and actual archives. Windows
   runtime of these new packages remains a later gate.
5. Cite the measurement in the PR.
   No performance delta can be cited before a controlled experiment.
6. Run Opening a PR.
   Skip, the user requires local commits only, no push or PR.

Architect phases are Ground, Sketch, Agree, Implement and Scrap. Keep the existing
monitoring harness and package validators. Preferred sketch is two isolated trees,
one generated frontend copied byte-for-byte, pinned helper payload, separate Cargo
targets and a comparison-only packager. Rejected alternative is changing runtime
feature flags inside one application build. That would add app behavior and risk
measuring a configuration difference. No new runtime types or public interface are
needed. The existing Vec/Arc and optional-metadata types are restored only in A.

Checkpoint after each unit: source diff and preservation hashes; common/variant
behavior tests; paired build inputs; actual compiled/package equality. Stop at a
failed gate. Do not repair unrelated app behavior to make this experiment pass.

## Future one-mode experiment, not started

The previous 90-second B pilot on example hybrid CPU used statusbar, app refresh 3 seconds,
sampler cadence 1 second, and a Fob process already about 38 minutes old. It is not
one leg of a matched pair against a freshly restarted A. If fresh B bytes differ,
it is also not a run of the newly paired build. Never combine it into controlled A/B.

After package review and explicit confirmation to exit/relaunch Fob, start with
statusbar only. Keep the same machine, ordinary token, app refresh 3000 ms, sampler
cadence 1000 ms, helper state, settings, visible windows, foreground/occlusion,
power state and background workload. Do not run A and B concurrently. Both use the
existing com.hcb.fob settings location, so do not change settings between runs.

Use a fresh app instance for every run. Give every instance the same 120-second
settle after attach, targeting attachment at app age 10 seconds (tolerance +/- 2
seconds, verified from OS creation time for every run),
then collect 180 seconds steady. Verify equivalent data freshness and functionality
before interpreting occupancy. Keep the helper unchanged and excluded. A two-run
A/B pretrial takes about 10 minutes plus launch time and is explicitly inconclusive.
Only after review, a single-mode A B A B A B A B A B series is 10 x 300 seconds,
about 50 minutes plus relaunch time, not the four-mode eight-hour matrix. If starting
with B, alternate B A instead. Use the same order rule throughout and record times.
Five complete runs per side and non-overlapping ranges are the sampler's minimum
for directional output, not proof of causality by themselves. Report medians,
ranges, sampler overhead and failures. Stop if noise exceeds the effect, data are
missing or workload/settings differ. A range-overlap result is inconclusive.

Private commit is not Task Manager's default private working set. Summed working
sets can repeat shared pages. Short slopes do not establish a leak. No build or
micro test establishes a whole-app improvement. Windows lifecycle/visibility and
runtime acceptance of both new archives remain pending.


The steady CPU mean is total included CPU seconds divided by 24 logical CPUs and
actual complete steady elapsed seconds, times 100. Do not compare raw one-core
percentages with that number. The sampler reports itself separately and excludes
the sensor helper. Keep its interval and quality gates identical on both sides.

After a future approved swap and read-only identity/mode verification, an operator
can use this template from the existing sampler directory. `$rootPid` must be the
newly verified Fob PID, not the old pilot PID. A1 is a new result directory; use the
matching label and a new directory for each subsequent run. Helper `unknown` is an
honest unchanged declaration, not proof of helper health.

```powershell
.\fob-perf.exe sample --pid $rootPid --out .\runs\A1 --label A --app-version 0.8.2 --mode statusbar --refresh-ms 3000 --helper unknown --interval-ms 1000 --settle-s 120 --steady-s 180
```

## Preparation verification, completed on the box

- A local commit: `55d03369d0387241bab56b50eb8a764c2ac85f5d`, worktree
  `/workspace/winperf-control-A-20261108`.
- B local commit: `14fc7a60f177ab1790867e67a172da7b0c29abe3`, worktree
  `/workspace/winperf-control-B-20261108`.
- Only the two monitoring production files differ between the variants. B release
  monitoring source equals the production pin, and A equals `ce2adfc^`. Shared
  characterization contracts are test-only. Main application source is untouched.
- Actual monitoring-harness tests: A 10 passed; B 17 passed. Seven shared contracts
  cover stable complete JSON, current dynamic counters and CPU normalization,
  timestamps, PID reuse, exits, optional metadata and actual collector copy policy.
  A's known old behavior is characterized explicitly, not represented as corrected.
- Existing release tests: 145 passed, plus metrics formatting passed. Standalone
  sampler: 39 passed. Frontend build passed; Svelte check: zero errors and warnings.
- Fresh offline locked Windows release builds and Windows all-target checks passed
  for both variants. `cargo xwin test --offline --locked --release --no-run
  --all-targets --target x86_64-pc-windows-msvc` also linked all Windows test
  targets for A and B successfully. These are cross-build checks, not Windows test
  execution. Release executable hashes were rechecked afterward.
- Full native main Rust tests could not start: `glib-sys` cannot find `pkg-config`.
  GTK/WebKit prerequisites remain unavailable. No dependency installation or
  network retry was attempted. No new GUI preview: frontend source and A/B assets
  are identical; Windows lifecycle and visibility gates remain unverified.
- Builds used cached Rust/Cargo 1.90.0, cargo-xwin 0.23.1, LLVM linker/resource
  compiler 23.1.4, Node 24.22.1 and npm 11.21.1. Release profile is opt-level 3,
  fat LTO, codegen-units 1, panic abort, strip true; STATIC_VCRUNTIME true and the
  unchanged `/IGNORE:4099` link argument. Separate target directories avoid mixed
  variants. Exact commands, compiler hashes, lockfile/config/icon/helper pins and
  generated frontend hashes are recorded in `build-inputs.json` and `audit.json`.
- Release logs retain compiler-family `clang-cl -E` probe warnings and the existing
  unused `mft_wire::frame` warning. Both builds finished successfully. Actual PEs
  are AMD64/Windows GUI and import no dynamic vcruntime/msvcp DLLs.
- Both packages passed HCB/version/icon, asInvoker, Common-Controls 6, embedded
  helper pin, original non-app payload, program checksum, ZIP CRC and extraction
  equality gates. Main FileVersion/ProductVersion is exactly 0.8.2. The unchanged
  managed sensor helper uses FileVersion 0.8.2.0 and ProductVersion 0.8.2.
- Two development audit assumptions were corrected, without application edits:
  optimized PEs construct the identifier from split instruction immediates (even
  the original PE has no contiguous identifier string), and the original managed
  helper has a four-component FileVersion. Identifier verification uses the pinned
  config and unchanged context generator. Initial failure logs are preserved.
- Repackaging into the same output was tested and refused with FileExistsError;
  both existing archive hashes remained unchanged. No originals were overwritten.
- New B is NOT byte-identical to the user's original B, even though application
  source is equivalent. The old pilot is sampler validation only, not a controlled
  run of this fresh pair. No measured optimization benefit or leak claim is made.
- No Windows/user-PC actions, app restart, settings/helper/driver changes, push,
  publication or PR occurred. Package review and future Windows acceptance remain
  required before using these comparison-only builds.

Evidence directory: `/workspace/winperf/dist/fob-cache-control-20261108/`.
Actual test/build stdout is retained in `harness-A.txt`, `harness-B.txt`,
`sampler-tests.txt`, `release-tests.txt`, `release-version.txt`, `frontend-B.txt`,
`frontend-check.txt`, `native-main-tests.txt`, `windows-release-A.txt`,
`windows-release-B.txt`, `windows-all-targets-A.txt`, `windows-all-targets-B.txt`
`windows-tests-no-run-A.txt`, `windows-tests-no-run-B.txt` and
`package-audit-passed.txt`. `variant-source.diff` records the complete variant
difference; `production-source-equivalence.diff` is empty as expected.

### Exact deliverables

Packages are under `dist/fob-cache-control-20261108/packages/`. Every archive
contains its variant/source/build manifest and concise Chinese comparison-only
instructions. Original production and sampler archives retain the hashes in the
comparison contract above.

- A: `fob-v0.8.2-control-A-windows.zip` — 8431562 bytes; SHA256
  `ccb64282989b45f2ad15d5e87379987a3288c330bb28e8f8e9a37e76936e6518`.
  Executable: 8327168 bytes; SHA256
  `62184b6221c486dfb3526a2bf52f14322af707530f23edb66f77faf79bd001a4`.
- B: `fob-v0.8.2-optimized-B-windows.zip` — 8432874 bytes; SHA256
  `8597c7f2b6965c1fedf4dcb75e3299ab16f40eb41203d7aab4d562a3e3e70abb`.
  Executable: 8331264 bytes; SHA256
  `de33e32d5f537d3958569b394b793fe51abcc7fdd566c50c5eefdfedd65e24a7`.
