//! One serialized source of preferences for all desktop windows.
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

/// Readings the status bar can show (METRICS in src/lib/utils/overlay.js).
/// The metrics with a history to chart (no video memory or GPU power);
/// CHART_METRICS in src/lib/utils/overlay.js.
pub const OVERLAY_CHART_METRICS: [&str; 7] =
    ["cpu", "cpuTemp", "gpu", "gpuTemp", "ram", "disk", "net"];
pub const OVERLAY_METRICS: [&str; 9] = [
    "cpu", "cpuTemp", "gpu", "gpuTemp", "ram", "vram", "disk", "net", "power",
];
/// Process column layout of this version. Settings saved before 0.8.3 have
/// none and stored GPU % as hidden, its old default, so it shows once.
const PROCESS_COLUMNS: u64 = 2;
fn migrate_columns(config: &mut Value, saved: &Value) {
    if saved["behavior"]["processColumns"].as_u64() != Some(PROCESS_COLUMNS) {
        config["appearance"]["columnVisibility"]["gpu_usage"] = Value::Bool(true);
    }
}
fn defaults() -> Value {
    serde_json::from_str(include_str!(
        "../../src/lib/definitions/default-config.json"
    ))
    .unwrap()
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    config: Value,
    revision: u64,
    ready: bool,
}
pub struct Preferences(pub Mutex<Snapshot>);
impl Default for Preferences {
    fn default() -> Self {
        Self(Mutex::new(Snapshot {
            config: defaults(),
            revision: 0,
            ready: false,
        }))
    }
}
// Only recognized fields are accepted. Old installations may have partial
// configs, so merge valid leaves into current defaults rather than replacing groups.
fn merge(config: &mut Value, patch: &Value) {
    for group in ["appearance", "behavior"] {
        let Some(values) = patch[group].as_object() else {
            continue;
        };
        for (key, value) in values {
            if group == "appearance" && key == "columnVisibility" {
                if let Some(columns) = value.as_object() {
                    for (column, visible) in columns {
                        if config[group][key].get(column).is_some() && visible.is_boolean() {
                            config[group][key][column] = visible.clone();
                        }
                    }
                }
                continue;
            }
            let valid = match key.as_str() {
                "itemsPerPage" => value
                    .as_u64()
                    .is_some_and(|n| [15, 25, 50, 100, 250, 500].contains(&n)),
                "refreshRate" => value.as_u64().is_some_and(|n| (1000..=30000).contains(&n)),
                "menuBarEnabled" | "keepRunning" | "startInTray" | "miniPinned"
                | "overlayEnabled" | "overlayLocked" | "launchAtLogin" => value.is_boolean(),
                // Fob status bar (overlay.rs, src/lib/utils/overlay.js).
                "overlayOpacity" => value.as_u64().is_some_and(|n| n <= 100),
                "overlayCharts" => value.is_boolean(),
                // 0.7.5: "" follows the main theme, else a theme name; the
                // frontend falls back to following for names it doesn't know.
                "overlayTheme" => value.as_str().is_some_and(|name| {
                    name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric())
                }),
                "overlayDock" => matches!(value.as_str(), Some("top" | "bottom" | "free")),
                // 0.7.3: the metrics drawn with a chart; until it's saved,
                // overlayCharts (0.7.1 on/off for all) stands in for it.
                "overlayChartMetrics" => value.as_array().is_some_and(|list| {
                    let ids: Vec<_> = list.iter().filter_map(Value::as_str).collect();
                    ids.len() == list.len()
                        && ids.iter().all(|id| OVERLAY_CHART_METRICS.contains(id))
                        && ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id))
                }),
                "overlayMetrics" => value.as_array().is_some_and(|list| {
                    let ids: Vec<_> = list.iter().filter_map(Value::as_str).collect();
                    ids.len() == list.len()
                        && ids.iter().all(|id| OVERLAY_METRICS.contains(id))
                        && ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id))
                }),
                // Fob: mini window opacity in percent (Settings and the mini window).
                "miniOpacity" => value.as_u64().is_some_and(|n| (0..=100).contains(&n)),
                "processView" => matches!(value.as_str(), Some("flat" | "tree")),
                // 0.8.3 process columns; a newer number is not ours to keep.
                "processColumns" => value.as_u64() == Some(PROCESS_COLUMNS),
                // Fob: "" (off) or a canonical combo; set via set_hotkey.
                "miniHotkey" | "overlayHotkey" | "mainHotkey" => value.as_str().is_some_and(|s| {
                    s.is_empty() || crate::hotkey::parse(s).is_ok_and(|c| c.canonical() == s)
                }),
                "language" => matches!(value.as_str(), Some("auto" | "en" | "zh")),
                "defaultStatusFilter" => matches!(
                    value.as_str(),
                    Some("all" | "running" | "sleeping" | "idle" | "unknown")
                ),
                _ => false,
            };
            if group == "behavior" && valid {
                config[group][key] = value.clone();
            }
            // Fob 0.7.1: the status bar opacity is the whole bar's now, 10 %
            // at least (0.7.0 allowed 0, which hid everything).
            if group == "behavior" && valid && key == "overlayOpacity" {
                config[group][key] = Value::from(value.as_u64().unwrap_or(60).max(10));
            }
        }
    }
    config["appearance"]["columnVisibility"]["name"] = Value::Bool(true);
}
fn persist(path: &Path, config: &Value) -> Result<(), String> {
    let parent = path.parent().ok_or("Settings location unavailable")?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create settings folder: {e}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Could not save settings: {e}"))?;
    fs::rename(&temporary, path).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        format!("Could not replace settings: {e}")
    })
}
fn load(path: &Path, legacy: Option<&Value>) -> Result<Option<Value>, String> {
    let existing = match fs::read(path) {
        Ok(bytes) => Some(
            serde_json::from_slice::<Value>(&bytes)
                .map_err(|e| format!("Could not read settings: {e}"))?,
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("Could not read settings: {e}")),
    };
    let Some(source) = existing.as_ref().or(legacy) else {
        return Ok(None);
    };
    let mut config = defaults();
    merge(&mut config, source);
    migrate_columns(&mut config, source);
    if existing.is_none() {
        persist(path, &config)?;
    }
    Ok(Some(config))
}
#[tauri::command]
pub async fn read_preferences(
    app: AppHandle,
    window: WebviewWindow,
    legacy: Option<Value>,
) -> Result<Snapshot, String> {
    let migrate = window.label() == "main";
    tauri::async_runtime::spawn_blocking(move || {
        let service = app.state::<Preferences>();
        let mut state = service.0.lock().map_err(|e| e.to_string())?;
        if !state.ready {
            let path = app
                .path()
                .app_config_dir()
                .map_err(|e| e.to_string())?
                .join("settings.json");
            if let Some(config) = load(&path, if migrate { legacy.as_ref() } else { None })? {
                state.config = config;
                state.ready = true;
                state.revision += 1;
                app.emit("preferences-change", state.clone())
                    .map_err(|e| e.to_string())?;
                let (app, config) = (app.clone(), state.config.clone());
                let login = config["behavior"]["launchAtLogin"]
                    .as_bool()
                    .unwrap_or(false);
                std::thread::spawn(move || {
                    crate::autostart::sync(login);
                    crate::overlay::sync(&app, &config);
                });
            }
        }
        Ok(state.clone())
    })
    .await
    .map_err(|e| e.to_string())?
}
/// Merge `patch` into the live preferences, persist and broadcast them.
pub fn apply_patch(app: &AppHandle, patch: &Value) -> Result<Snapshot, String> {
    let service = app.state::<Preferences>();
    let mut state = service.0.lock().map_err(|e| e.to_string())?;
    if !state.ready {
        return Err("Settings are still starting. Try again shortly.".into());
    }
    let mut next = state.config.clone();
    merge(&mut next, patch);
    if next != state.config {
        let path = app
            .path()
            .app_config_dir()
            .map_err(|e| e.to_string())?
            .join("settings.json");
        persist(&path, &next)?;
        state.config = next;
        state.revision += 1;
        app.emit("preferences-change", state.clone())
            .map_err(|e| e.to_string())?;
        let (app, config) = (app.clone(), state.config.clone());
        std::thread::spawn(move || crate::overlay::sync(&app, &config));
    }
    Ok(state.clone())
}
#[tauri::command]
pub async fn update_preferences(app: AppHandle, mut patch: Value) -> Result<Snapshot, String> {
    // Fob: the hotkey is only saved by set_hotkey, once it's registered
    // (pages send their whole behavior group, possibly with an old value).
    if let Some(behavior) = patch["behavior"].as_object_mut() {
        behavior.remove("miniHotkey");
        behavior.remove("overlayHotkey");
        behavior.remove("mainHotkey");
    }
    tauri::async_runtime::spawn_blocking(move || apply_patch(&app, &patch))
        .await
        .map_err(|e| e.to_string())?
}

/// winperf: what launch needs before any webview has loaded the settings.
#[derive(Debug, PartialEq)]
pub struct Startup {
    /// No settings file yet: this is the first launch.
    pub first_launch: bool,
    pub tray: bool,
    pub start_in_tray: bool,
    /// Fob: the mini window stays open when another window is clicked.
    pub mini_pinned: bool,
    /// "auto" | "en" | "zh" (validated by `merge`).
    pub language: Option<String>,
    /// Fob: the mini window hotkey, canonical ("" = off).
    pub mini_hotkey: String,
    /// Fob: the status bar hotkey, canonical ("" = off).
    pub overlay_hotkey: String,
    /// Fob 0.7.12: the full window hotkey, canonical ("" = off).
    pub main_hotkey: String,
    /// Fob 0.7.9: start with the Windows session (autostart.rs).
    pub launch_at_login: bool,
}
pub fn startup_from(saved: Option<&Value>) -> Startup {
    let mut config = defaults();
    if let Some(saved) = saved {
        merge(&mut config, saved);
    }
    let flag = |key: &str| config["behavior"][key].as_bool().unwrap_or(true);
    Startup {
        first_launch: saved.is_none(),
        tray: flag("menuBarEnabled"),
        start_in_tray: flag("startInTray"),
        mini_pinned: config["behavior"]["miniPinned"].as_bool().unwrap_or(false),
        language: config["behavior"]["language"].as_str().map(str::to_owned),
        mini_hotkey: config["behavior"]["miniHotkey"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        main_hotkey: config["behavior"]["mainHotkey"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        overlay_hotkey: config["behavior"]["overlayHotkey"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        launch_at_login: config["behavior"]["launchAtLogin"]
            .as_bool()
            .unwrap_or(false),
    }
}
pub fn startup(app: &AppHandle) -> Startup {
    let saved = app
        .path()
        .app_config_dir()
        .ok()
        .and_then(|dir| fs::read(dir.join("settings.json")).ok())
        .map(|bytes| serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null));
    startup_from(saved.as_ref())
}
#[cfg(test)]
mod tests {
    use super::*;
    /// The source without its `#[cfg(test)]` items. rustfmt puts the end of
    /// a top-level item at column 0, which is where each one stops.
    fn production_code(src: &str) -> String {
        let mut code = String::new();
        let mut in_test = false;
        for line in src.lines() {
            if line == "#[cfg(test)]" {
                in_test = true;
            } else if !in_test {
                code.push_str(line);
                code.push('\n');
            } else if line == "}" || (!line.starts_with([' ', '#']) && line.ends_with(';')) {
                in_test = false;
            }
        }
        code
    }

    /// Bundle identifiers look like com.vendor.app. The only one allowed
    /// in production code is the current identifier, so a retired folder
    /// cannot return. Test examples do not create folders, so they are skipped.
    fn unexpected_app_identifiers(src: &str) -> Vec<String> {
        const CURRENT: &str = "com.hcb.fob";
        let src = production_code(src);
        let src = src.as_str();
        let mut found = Vec::new();
        let mut from = 0;
        while let Some(rel) = src[from..].find("com.") {
            let start = from + rel;
            let rest = &src[start + 4..];
            let first = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .count();
            if first == 0 {
                from = start + 4;
                continue;
            }
            let mut end = start + 4 + first;
            while end < src.len() && src.as_bytes()[end] == b'.' {
                let label = &src[end + 1..];
                let n = label
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .count();
                if n == 0 {
                    break;
                }
                end += 1 + n;
            }
            let token = &src[start..end];
            if token != CURRENT {
                found.push(token.to_string());
            }
            from = end;
        }
        found
    }

    #[test]
    fn the_identifier_guard_skips_only_test_items() {
        let src = "fn a() {\n    \"com.old.app\";\n}\n#[cfg(test)]\nmod tests {\n    \"com.vendor.app\";\n}\n#[cfg(test)]\nconst X: &str = \"com.fixture.app\";\nfn b() {\n    \"com.hcb.fob\";\n    \"com.other.app\";\n}\n";
        assert_eq!(
            unexpected_app_identifiers(src),
            ["com.old.app", "com.other.app"]
        );
        assert_eq!(
            unexpected_app_identifiers(&src.replace('\n', "\r\n")),
            ["com.old.app", "com.other.app"]
        );
    }

    #[test]
    fn folders_follow_the_identifier_and_no_debug_log() {
        let conf: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], "com.hcb.fob");
        for (name, src) in [
            ("boot.rs", include_str!("boot.rs")),
            ("preferences.rs", include_str!("preferences.rs")),
            ("companion.rs", include_str!("companion.rs")),
            ("overlay.rs", include_str!("overlay.rs")),
            ("space.rs", include_str!("space.rs")),
            ("main.rs", include_str!("main.rs")),
        ] {
            let unexpected = unexpected_app_identifiers(src);
            assert!(
                unexpected.is_empty(),
                "{name}: unexpected identifier {unexpected:?}"
            );
            assert!(
                !src.contains(concat!("fob", ".log")),
                "{name}: no debug log file"
            );
        }
    }
    use serde_json::json;
    #[test]
    fn legacy_partial_configs_keep_defaults_and_reject_invalid_values() {
        let mut config = defaults();
        merge(
            &mut config,
            &json!({"behavior":{"refreshRate":5000,"keepRunning":true,"itemsPerPage":0,"processView":"bad"},"appearance":{"columnVisibility":{"name":false,"pid":false,"unknown":true}}}),
        );
        assert_eq!(config["behavior"]["refreshRate"], 5000);
        assert_eq!(config["behavior"]["itemsPerPage"], 15);
        assert_eq!(config["behavior"]["processView"], "flat");
        assert_eq!(config["appearance"]["columnVisibility"]["name"], true);
        assert_eq!(config["appearance"]["columnVisibility"]["pid"], false);
        assert!(config["appearance"]["columnVisibility"]
            .get("unknown")
            .is_none());
    }
    #[test]
    fn gpu_column_shows_once_for_settings_from_before_0_8_3() {
        let saved = json!({"behavior":{"refreshRate":3000},"appearance":{"columnVisibility":{"gpu_usage":false,"disk_usage":true,"virtual_memory":false}}});
        let mut config = defaults();
        merge(&mut config, &saved);
        migrate_columns(&mut config, &saved);
        let columns = &config["appearance"]["columnVisibility"];
        assert_eq!(columns["gpu_usage"], true);
        assert_eq!(columns["disk_read"], true);
        assert_eq!(columns["disk_write"], true);
        assert_eq!(columns["gpu_memory"], false);
        assert_eq!(columns["virtual_memory"], false);
        assert!(columns.get("disk_usage").is_none());
        assert_eq!(config["behavior"]["processColumns"], 2);
        // Hidden again after 0.8.3: the choice sticks across restarts.
        let saved = json!({"behavior":{"processColumns":2},"appearance":{"columnVisibility":{"gpu_usage":false}}});
        let mut config = defaults();
        merge(&mut config, &saved);
        migrate_columns(&mut config, &saved);
        assert_eq!(config["appearance"]["columnVisibility"]["gpu_usage"], false);
        // A later patch never re-applies the migration.
        merge(&mut config, &json!({"behavior":{"language":"zh"}}));
        assert_eq!(config["appearance"]["columnVisibility"]["gpu_usage"], false);
    }
    #[test]
    fn status_bar_settings_default_off_and_reject_bad_values() {
        let mut config = defaults();
        let b = |c: &Value, k: &str| c["behavior"][k].clone();
        assert_eq!(b(&config, "overlayEnabled"), false);
        assert_eq!(b(&config, "overlayDock"), "top");
        assert_eq!(b(&config, "overlayLocked"), false);
        assert_eq!(b(&config, "overlayOpacity"), 60);
        merge(
            &mut config,
            &json!({"behavior":{"overlayEnabled":true,"overlayDock":"free","overlayOpacity":35,
                "overlayLocked":true,"overlayMetrics":["net","cpu"]}}),
        );
        assert_eq!(b(&config, "overlayLocked"), true);
        assert_eq!(b(&config, "overlayEnabled"), true);
        assert_eq!(b(&config, "overlayDock"), "free");
        assert_eq!(b(&config, "overlayOpacity"), 35);
        assert_eq!(b(&config, "overlayMetrics"), json!(["net", "cpu"]));
        for bad in [
            json!({"overlayDock":"left"}),
            json!({"overlayOpacity":101}),
            json!({"overlayOpacity":-1}),
            json!({"overlayCharts":1}),
            json!({"overlayEnabled":"yes"}),
            json!({"overlayMetrics":["cpu","cpu"]}),
            json!({"overlayMetrics":["fps"]}),
            json!({"overlayMetrics":"cpu"}),
            json!({"overlayChartMetrics":["vram"]}),
            json!({"overlayChartMetrics":["power"]}),
            json!({"overlayChartMetrics":["cpu","cpu"]}),
            json!({"overlayChartMetrics":true}),
            json!({"overlayTheme":1}),
            json!({"overlayTheme":"../x"}),
        ] {
            let before = config.clone();
            merge(&mut config, &json!({ "behavior": bad }));
            assert_eq!(config, before, "{bad} must be ignored");
        }
        merge(
            &mut config,
            &json!({"behavior":{"overlayChartMetrics":["net","cpu"]}}),
        );
        assert_eq!(b(&config, "overlayChartMetrics"), json!(["net", "cpu"]));
        merge(&mut config, &json!({"behavior":{"overlayChartMetrics":[]}}));
        assert_eq!(
            b(&config, "overlayTheme"),
            "",
            "follows the main window by default"
        );
        merge(&mut config, &json!({"behavior":{"overlayTheme":"dracula"}}));
        assert_eq!(b(&config, "overlayTheme"), "dracula");
        merge(&mut config, &json!({"behavior":{"overlayTheme":""}}));
        assert_eq!(b(&config, "overlayTheme"), "");
        assert_eq!(b(&config, "overlayChartMetrics"), json!([]));
        merge(&mut config, &json!({"behavior":{"overlayOpacity":3}}));
        assert_eq!(
            b(&config, "overlayOpacity"),
            10,
            "0.7.0 values under 10 % move up to 10 %"
        );
        merge(&mut config, &json!({"behavior":{"overlayOpacity":35}}));
        assert_eq!(b(&config, "overlayCharts"), false);
        merge(&mut config, &json!({"behavior":{"overlayCharts":true}}));
        assert_eq!(b(&config, "overlayCharts"), true);
        assert_eq!(b(&config, "overlayHotkey"), "");
        merge(
            &mut config,
            &json!({"behavior":{"overlayHotkey":"Ctrl+Alt+H"}}),
        );
        assert_eq!(b(&config, "overlayHotkey"), "Ctrl+Alt+H");
        merge(&mut config, &json!({"behavior":{"overlayHotkey":"H"}}));
        assert_eq!(
            b(&config, "overlayHotkey"),
            "Ctrl+Alt+H",
            "no modifier: refused"
        );
        merge(&mut config, &json!({"behavior":{"overlayMetrics":[]}}));
        assert_eq!(b(&config, "overlayMetrics"), json!([]));
    }
    #[test]
    fn the_mini_hotkey_is_off_by_default_and_only_saved_canonical() {
        let mut config = defaults();
        assert_eq!(config["behavior"]["miniHotkey"], "");
        merge(
            &mut config,
            &json!({"behavior":{"miniHotkey":"Ctrl+Alt+F"}}),
        );
        assert_eq!(config["behavior"]["miniHotkey"], "Ctrl+Alt+F");
        for bad in [json!("ctrl+alt+f"), json!("F"), json!(5)] {
            merge(&mut config, &json!({"behavior":{"miniHotkey":bad}}));
            assert_eq!(config["behavior"]["miniHotkey"], "Ctrl+Alt+F");
        }
        merge(&mut config, &json!({"behavior":{"miniHotkey":""}}));
        assert_eq!(config["behavior"]["miniHotkey"], "");
        let saved = startup_from(Some(&json!({"behavior":{"miniHotkey":"Ctrl+Shift+F"}})));
        assert_eq!(saved.mini_hotkey, "Ctrl+Shift+F");
    }
    #[test]
    fn start_in_tray_defaults_on_for_new_and_existing_installs() {
        let first = startup_from(None);
        assert_eq!(
            first,
            Startup {
                first_launch: true,
                tray: true,
                start_in_tray: true,
                mini_pinned: false,
                language: Some("auto".into()),
                mini_hotkey: String::new(),
                overlay_hotkey: String::new(),
                main_hotkey: String::new(),
                launch_at_login: false,
            }
        );
        // Fob 0.7.9: login start is off until turned on, and only a boolean.
        let on = startup_from(Some(&json!({"behavior":{"launchAtLogin":true}})));
        assert!(on.launch_at_login);
        let bad = startup_from(Some(&json!({"behavior":{"launchAtLogin":"yes"}})));
        assert!(!bad.launch_at_login);
        // v0.3 settings files have no startInTray key: still on.
        let old = startup_from(Some(&json!({"behavior":{"refreshRate":5000}})));
        assert!(!old.first_launch && old.start_in_tray && old.tray);
        let off = startup_from(Some(
            &json!({"behavior":{"startInTray":false,"menuBarEnabled":false}}),
        ));
        assert!(!off.start_in_tray && !off.tray);
        // Language: valid values kept, anything else falls back to "auto".
        let zh = startup_from(Some(&json!({"behavior":{"language":"zh"}})));
        assert_eq!(zh.language.as_deref(), Some("zh"));
        let bad = startup_from(Some(&json!({"behavior":{"language":"klingon"}})));
        assert_eq!(bad.language.as_deref(), Some("auto"));
        // Unreadable file: treat as existing with defaults.
        assert!(startup_from(Some(&Value::Null)).start_in_tray);
    }
    #[test]
    fn mini_opacity_is_opaque_by_default_and_kept_in_range() {
        let mut config = defaults();
        assert_eq!(config["behavior"]["miniOpacity"], 100);
        merge(&mut config, &json!({"behavior":{"miniOpacity":70}}));
        assert_eq!(config["behavior"]["miniOpacity"], 70);
        merge(&mut config, &json!({"behavior":{"miniOpacity":0}}));
        assert_eq!(config["behavior"]["miniOpacity"], 0);
        merge(&mut config, &json!({"behavior":{"miniOpacity":70}}));
        for bad in [json!(101), json!(70.5), json!("70"), json!(-1)] {
            merge(&mut config, &json!({"behavior":{"miniOpacity":bad}}));
            assert_eq!(config["behavior"]["miniOpacity"], 70);
        }
    }
    #[test]
    fn mini_pin_survives_a_restart() {
        // What set_companion_pinned saves is what the next launch reads.
        let mut config = defaults();
        assert_eq!(config["behavior"]["miniPinned"], json!(false));
        merge(&mut config, &json!({"behavior":{"miniPinned":true}}));
        assert!(startup_from(Some(&config)).mini_pinned);
        merge(&mut config, &json!({"behavior":{"miniPinned":"yes"}}));
        assert!(startup_from(Some(&config)).mini_pinned, "non-bool rejected");
        merge(&mut config, &json!({"behavior":{"miniPinned":false}}));
        assert!(!startup_from(Some(&config)).mini_pinned);
        // Older settings files have no key: unpinned.
        assert!(!startup_from(Some(&json!({"behavior":{}}))).mini_pinned);
    }
    #[test]
    fn independent_patches_do_not_overwrite_other_windows() {
        let mut config = defaults();
        merge(&mut config, &json!({"behavior":{"refreshRate":5000}}));
        merge(&mut config, &json!({"behavior":{"processView":"tree"}}));
        merge(
            &mut config,
            &json!({"appearance":{"columnVisibility":{"pid":false}}}),
        );
        assert_eq!(config["behavior"]["refreshRate"], 5000);
        assert_eq!(config["behavior"]["processView"], "tree");
    }
    #[test]
    fn saved_settings_win_over_legacy_and_failed_save_keeps_original() {
        let dir = std::env::temp_dir().join(format!("neohtop-preferences-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert!(load(&path, None).unwrap().is_none());
        let config = load(&path, Some(&json!({"behavior":{"refreshRate":5000}})))
            .unwrap()
            .unwrap();
        assert_eq!(load(&path, Some(&defaults())).unwrap().unwrap(), config);
        assert!(persist(&dir, &defaults()).is_err());
        assert_eq!(load(&path, None).unwrap().unwrap(), config);
        fs::remove_dir_all(dir).unwrap();
    }
}
