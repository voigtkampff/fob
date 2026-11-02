//! winperf: tray menu strings (the webview UI has its own tables in
//! src/lib/i18n/strings.js). Same rule: "auto" follows the Windows display
//! language, Chinese for zh*, English otherwise.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Zh,
}

pub fn resolve(pref: Option<&str>, system: Option<&str>) -> Locale {
    match pref {
        Some("en") => Locale::En,
        Some("zh") => Locale::Zh,
        _ => match system {
            Some(tag) if tag.to_ascii_lowercase().starts_with("zh") => Locale::Zh,
            _ => Locale::En,
        },
    }
}

/// The user's display language (Windows: GetUserPreferredUILanguages).
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

pub struct Tray {
    pub show_mini: &'static str,
    pub open_full: &'static str,
    pub start_in_tray: &'static str,
    pub language: &'static str,
    pub auto: &'static str,
    pub quit: &'static str,
    pub tooltip: &'static str,
}

pub fn tray(locale: Locale) -> Tray {
    match locale {
        Locale::En => Tray {
            show_mini: "Show mini window",
            open_full: "Open full window",
            start_in_tray: "Start in tray only (mini window mode)",
            language: "Language",
            auto: "Automatic (Windows)",
            quit: "Quit",
            tooltip: "winperf — system overview",
        },
        Locale::Zh => Tray {
            show_mini: "显示迷你窗口",
            open_full: "打开完整窗口",
            start_in_tray: "仅在托盘启动（迷你窗口模式）",
            language: "语言",
            auto: "自动（跟随 Windows）",
            quit: "退出",
            tooltip: "winperf — 系统概览",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_system_language() {
        assert_eq!(resolve(Some("auto"), Some("zh-CN")), Locale::Zh);
        assert_eq!(resolve(Some("auto"), Some("zh-Hant-TW")), Locale::Zh);
        assert_eq!(resolve(None, Some("en-US")), Locale::En);
        assert_eq!(resolve(Some("auto"), Some("de-DE")), Locale::En);
        assert_eq!(resolve(Some("auto"), None), Locale::En);
        assert_eq!(resolve(Some("en"), Some("zh-CN")), Locale::En);
        assert_eq!(resolve(Some("zh"), Some("en-US")), Locale::Zh);
        assert_eq!(resolve(Some("fr"), Some("zh-CN")), Locale::Zh);
    }

    #[test]
    fn both_tray_tables_are_filled() {
        for locale in [Locale::En, Locale::Zh] {
            let t = tray(locale);
            for text in [
                t.show_mini,
                t.open_full,
                t.start_in_tray,
                t.language,
                t.auto,
                t.quit,
                t.tooltip,
            ] {
                assert!(!text.trim().is_empty());
            }
        }
        assert_ne!(tray(Locale::En).quit, tray(Locale::Zh).quit);
    }
}
