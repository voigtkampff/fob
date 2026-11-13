# Helper status lifecycle

Run `node .cursor/skills/verify-fob/bin/helper-settings.mjs zh <evidence-dir>` and repeat with `en`.

Uses the existing helper-settings preview with the original SettingsWindow and SensorSetup components. Tauri status and setup are mocked. No driver or helper is installed.

Checks one initial status request under sequential and overlapping IPC timing, focus/reopen and manual refresh, General pane remount, explicit install/update/repair/remove consent, action success/cancel/error refresh, and query-error recovery. Saves counts and a Settings screenshot for each locale. This is synthetic frontend evidence, not packaged Windows runtime evidence.

CPU availability regression: run `node .cursor/skills/verify-fob/bin/process-table.mjs zh <evidence-dir>` and repeat with `en` for zero, missing, recovered and partial-tree readings. Native Windows refresh policy and CPU samples are covered by `cargo test --locked --manifest-path src-tauri/Cargo.toml` in the existing Build Check workflow.
