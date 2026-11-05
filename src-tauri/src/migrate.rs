//! Fob 0.7.14: the app identifier changed from com.hcb.fob to com.hcb.fob,
//! which moves every per-user folder Tauri derives from it. Once, on launch,
//! the saved settings and window positions are copied from the old folders
//! when the new ones lack them. The old folders are left alone (WebView2's
//! cache lives there; the user may delete them).
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Identifier before 0.7.14.
pub const OLD_IDENTIFIER: &str = "com.hcb.fob";
/// Files that hold what the user set; everything else there is a cache or log.
pub const FILES: [&str; 3] = [
    "settings.json",
    "overlay-position.json",
    "mini-position.json",
];

/// Copy each of `FILES` missing from `new` from the first old folder that has
/// it. Returns the names copied. Never overwrites, never deletes.
pub fn copy_missing(old_dirs: &[PathBuf], new: &Path) -> Vec<&'static str> {
    let mut copied = Vec::new();
    for name in FILES {
        if new.join(name).exists() {
            continue;
        }
        let Some(from) = old_dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) else {
            continue;
        };
        if std::fs::create_dir_all(new).is_ok() && std::fs::copy(&from, new.join(name)).is_ok() {
            copied.push(name);
        }
    }
    copied
}

/// The same folder under the old identifier: `…\com.hcb.fob` → `…\com.hcb.fob`.
fn renamed(dir: &Path, identifier: &str) -> Option<PathBuf> {
    (dir.file_name()? == identifier).then(|| dir.with_file_name(OLD_IDENTIFIER))
}

/// Run before anything reads settings. Roaming (%APPDATA%, app_config_dir)
/// first, then local (%LOCALAPPDATA%).
pub fn run(app: &AppHandle) {
    let id = &app.config().identifier;
    let path = app.path();
    let Ok(new) = path.app_config_dir() else {
        return;
    };
    let old: Vec<PathBuf> = [path.app_config_dir(), path.app_local_data_dir()]
        .into_iter()
        .filter_map(|d| renamed(&d.ok()?, id))
        .collect();
    copy_missing(&old, &new);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_only_what_the_new_folder_lacks_and_keeps_the_old() {
        let root = std::env::temp_dir().join(format!("fob-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (roaming, local, new) = (root.join("roam"), root.join("local"), root.join("new"));
        for d in [&roaming, &local, &new] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(roaming.join("settings.json"), "old roaming").unwrap();
        std::fs::write(local.join("settings.json"), "old local").unwrap();
        std::fs::write(local.join("mini-position.json"), "mini").unwrap();
        std::fs::write(roaming.join("overlay-position.json"), "old bar").unwrap();
        std::fs::write(new.join("overlay-position.json"), "new bar").unwrap();
        std::fs::write(roaming.join("fob.log"), "debug").unwrap();
        let copied = copy_missing(&[roaming.clone(), local.clone()], &new);
        assert_eq!(copied, ["settings.json", "mini-position.json"]);
        let read = |p: PathBuf| std::fs::read_to_string(p).unwrap();
        assert_eq!(
            read(new.join("settings.json")),
            "old roaming",
            "roaming wins"
        );
        assert_eq!(read(new.join("mini-position.json")), "mini");
        assert_eq!(
            read(new.join("overlay-position.json")),
            "new bar",
            "never overwrites"
        );
        assert!(!new.join("fob.log").exists(), "the debug log stays behind");
        assert!(
            roaming.join("settings.json").exists(),
            "the old folder is kept"
        );
        assert!(
            copy_missing(&[roaming, local], &new).is_empty(),
            "second launch: nothing"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn the_old_folder_is_the_new_one_renamed() {
        let new = Path::new("/users/me/AppData/Roaming/com.hcb.fob");
        assert_eq!(
            renamed(new, "com.hcb.fob").unwrap(),
            Path::new("/users/me/AppData/Roaming/com.hcb.fob")
        );
        assert!(renamed(Path::new("/x/other"), "com.hcb.fob").is_none());
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], "com.hcb.fob");
    }
    #[test]
    fn no_production_path_names_the_old_identifier() {
        for (name, src) in [
            ("boot.rs", include_str!("boot.rs")),
            ("preferences.rs", include_str!("preferences.rs")),
            ("companion.rs", include_str!("companion.rs")),
            ("overlay.rs", include_str!("overlay.rs")),
            ("space.rs", include_str!("space.rs")),
            ("main.rs", include_str!("main.rs")),
            ("tauri.conf.json", include_str!("../tauri.conf.json")),
            ("Cargo.toml", include_str!("../Cargo.toml")),
        ] {
            assert!(!src.contains("the owner") && !src.contains("the owner"), "{name}");
            assert!(!src.contains("fob.log"), "{name}: no debug log file");
        }
    }
}
