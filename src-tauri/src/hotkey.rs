//! Fob: the global hotkey that toggles the tray mini window.
//!
//! Registered through tauri-plugin-global-shortcut, which uses
//! RegisterHotKey on Windows (no keyboard hook). The combo is saved in
//! behavior.miniHotkey as a canonical string like "Ctrl+Alt+F"; "" = off.
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// A parsed combo: modifiers in canonical order plus one key.
#[derive(Clone, Debug, PartialEq)]
pub struct Combo {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Canonical key name: "A"–"Z", "0"–"9", "F1"–"F24", "Space", …
    pub key: String,
}

/// Why a combo was refused; the Settings page shows a message per code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refused {
    /// Not a combo Fob understands (unknown key, two keys, empty part).
    Invalid,
    /// Needs Ctrl, Alt or Win (Shift alone only with F1–F24).
    NeedsModifier,
    /// Another app (or Windows) already owns it.
    Taken,
}
impl Refused {
    pub fn code(self) -> &'static str {
        match self {
            Refused::Invalid => "invalid",
            Refused::NeedsModifier => "needsModifier",
            Refused::Taken => "taken",
        }
    }
}

const NAMED: &[(&str, Code)] = &[
    ("Space", Code::Space),
    ("Up", Code::ArrowUp),
    ("Down", Code::ArrowDown),
    ("Left", Code::ArrowLeft),
    ("Right", Code::ArrowRight),
    ("Home", Code::Home),
    ("End", Code::End),
    ("PageUp", Code::PageUp),
    ("PageDown", Code::PageDown),
    ("Insert", Code::Insert),
    ("Delete", Code::Delete),
    ("`", Code::Backquote),
    ("-", Code::Minus),
    ("=", Code::Equal),
    ("[", Code::BracketLeft),
    ("]", Code::BracketRight),
    (";", Code::Semicolon),
    ("'", Code::Quote),
    (",", Code::Comma),
    (".", Code::Period),
    ("/", Code::Slash),
    ("\\", Code::Backslash),
];
const LETTERS: [Code; 26] = [
    Code::KeyA,
    Code::KeyB,
    Code::KeyC,
    Code::KeyD,
    Code::KeyE,
    Code::KeyF,
    Code::KeyG,
    Code::KeyH,
    Code::KeyI,
    Code::KeyJ,
    Code::KeyK,
    Code::KeyL,
    Code::KeyM,
    Code::KeyN,
    Code::KeyO,
    Code::KeyP,
    Code::KeyQ,
    Code::KeyR,
    Code::KeyS,
    Code::KeyT,
    Code::KeyU,
    Code::KeyV,
    Code::KeyW,
    Code::KeyX,
    Code::KeyY,
    Code::KeyZ,
];
const DIGITS: [Code; 10] = [
    Code::Digit0,
    Code::Digit1,
    Code::Digit2,
    Code::Digit3,
    Code::Digit4,
    Code::Digit5,
    Code::Digit6,
    Code::Digit7,
    Code::Digit8,
    Code::Digit9,
];
const FKEYS: [Code; 24] = [
    Code::F1,
    Code::F2,
    Code::F3,
    Code::F4,
    Code::F5,
    Code::F6,
    Code::F7,
    Code::F8,
    Code::F9,
    Code::F10,
    Code::F11,
    Code::F12,
    Code::F13,
    Code::F14,
    Code::F15,
    Code::F16,
    Code::F17,
    Code::F18,
    Code::F19,
    Code::F20,
    Code::F21,
    Code::F22,
    Code::F23,
    Code::F24,
];

/// The key's canonical name and code, case-insensitive.
fn key_code(name: &str) -> Option<(String, Code)> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii_uppercase() {
        return Some((upper.clone(), LETTERS[(bytes[0] - b'A') as usize]));
    }
    if bytes.len() == 1 && bytes[0].is_ascii_digit() {
        return Some((upper.clone(), DIGITS[(bytes[0] - b'0') as usize]));
    }
    if let Some(n) = upper
        .strip_prefix('F')
        .and_then(|n| n.parse::<usize>().ok())
    {
        if (1..=24).contains(&n) && !upper[1..].starts_with('0') {
            return Some((format!("F{n}"), FKEYS[n - 1]));
        }
    }
    NAMED
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(n, c)| (n.to_string(), *c))
}

/// Parses "ctrl + alt + f", "Control+Shift+F5", "Win+Space"… The last part is
/// the key, the others modifiers (each at most once).
pub fn parse(text: &str) -> Result<Combo, Refused> {
    let parts: Vec<&str> = text.split('+').map(str::trim).collect();
    // "Ctrl++" style: a literal "+" key is not supported.
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) {
        return Err(if parts.len() == 1 && key_code(parts[0]).is_some() {
            Refused::NeedsModifier
        } else {
            Refused::Invalid
        });
    }
    let (key, mods) = parts.split_last().unwrap();
    let mut combo = Combo {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
        key: String::new(),
    };
    for m in mods {
        let flag = match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => &mut combo.ctrl,
            "alt" | "option" => &mut combo.alt,
            "shift" => &mut combo.shift,
            "win" | "super" | "meta" | "cmd" | "command" => &mut combo.win,
            _ => return Err(Refused::Invalid),
        };
        if *flag {
            return Err(Refused::Invalid);
        }
        *flag = true;
    }
    combo.key = key_code(key).ok_or(Refused::Invalid)?.0;
    let function_key = combo.key.len() > 1 && combo.key.starts_with('F');
    if !(combo.ctrl || combo.alt || combo.win || (combo.shift && function_key)) {
        return Err(Refused::NeedsModifier);
    }
    Ok(combo)
}

impl Combo {
    /// "Ctrl+Alt+Shift+Win+Key".
    pub fn canonical(&self) -> String {
        let mut out = Vec::new();
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.win, "Win"),
        ] {
            if on {
                out.push(name);
            }
        }
        out.push(&self.key);
        out.join("+")
    }
    pub fn shortcut(&self) -> Shortcut {
        let mut mods = Modifiers::empty();
        mods.set(Modifiers::CONTROL, self.ctrl);
        mods.set(Modifiers::ALT, self.alt);
        mods.set(Modifiers::SHIFT, self.shift);
        mods.set(Modifiers::SUPER, self.win);
        // `parse` only accepts keys `key_code` knows.
        Shortcut::new(Some(mods), key_code(&self.key).map(|k| k.1).unwrap())
    }
}

/// The registered combo ("" = none) and the saved combo that failed to
/// register at launch, with why.
#[derive(Default)]
pub struct HotkeyState(Mutex<(String, Option<(&'static str, String)>)>);

/// The plugin, with the handler that toggles the mini window.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::companion::toggle(app);
            }
        })
        .build()
}

/// Registers `next` (canonical, "" = off) in place of the current combo.
/// The current one stays registered if `next` can't be.
fn swap(app: &AppHandle, next: &str) -> Result<(), Refused> {
    let state = app.state::<HotkeyState>();
    let mut current = state.0.lock().map_err(|_| Refused::Invalid)?;
    if current.0 == next {
        return Ok(());
    }
    let shortcuts = app.global_shortcut();
    if !next.is_empty() {
        let combo = parse(next)?;
        shortcuts
            .register(combo.shortcut())
            .map_err(|_| Refused::Taken)?;
    }
    if let Ok(old) = parse(&current.0) {
        if shortcuts.is_registered(old.shortcut()) {
            let _ = shortcuts.unregister(old.shortcut());
        }
    }
    current.0 = next.to_owned();
    Ok(())
}

/// At launch (also tray-only): register the saved combo.
pub fn register_saved(app: &AppHandle, saved: &str) {
    if saved.is_empty() {
        return;
    }
    let result = parse(saved).and_then(|c| swap(app, &c.canonical()));
    if let (Err(refused), Ok(mut state)) = (result, app.state::<HotkeyState>().0.lock()) {
        state.1 = Some((refused.code(), saved.to_owned()));
    }
}

#[derive(serde::Serialize)]
pub struct HotkeyStatus {
    combo: String,
    /// Why the saved combo couldn't be registered at launch.
    error: Option<&'static str>,
    /// That saved combo (still saved; registering it is retried next launch).
    failed: String,
}
#[tauri::command]
pub fn get_mini_hotkey(app: AppHandle) -> HotkeyStatus {
    let state = app.state::<HotkeyState>();
    let state = state.0.lock();
    let (combo, failure) = state.map(|s| s.clone()).unwrap_or_default();
    let (error, failed) = match failure {
        Some((code, combo)) => (Some(code), combo),
        None => (None, String::new()),
    };
    HotkeyStatus {
        combo,
        error,
        failed,
    }
}
/// While Settings records a new combo the current one is let go, so pressing
/// it again reaches the recorder instead of toggling the mini window.
#[tauri::command]
pub fn suspend_mini_hotkey(app: AppHandle, suspend: bool) {
    let current = app
        .state::<HotkeyState>()
        .0
        .lock()
        .map(|s| s.0.clone())
        .unwrap_or_default();
    if let Ok(combo) = parse(&current) {
        let shortcuts = app.global_shortcut();
        let registered = shortcuts.is_registered(combo.shortcut());
        if suspend && registered {
            let _ = shortcuts.unregister(combo.shortcut());
        } else if !suspend && !registered {
            let _ = shortcuts.register(combo.shortcut());
        }
    }
}
/// Sets (or clears, with "") the hotkey and saves it. Err is a code:
/// "invalid" | "needsModifier" | "taken"; the previous combo is kept.
#[tauri::command]
pub fn set_mini_hotkey(app: AppHandle, combo: String) -> Result<String, String> {
    let next = if combo.trim().is_empty() {
        String::new()
    } else {
        parse(&combo).map_err(|r| r.code().to_owned())?.canonical()
    };
    swap(&app, &next).map_err(|r| r.code().to_owned())?;
    if let Ok(mut state) = app.state::<HotkeyState>().0.lock() {
        state.1 = None;
    }
    crate::preferences::apply_patch(&app, &serde_json::json!({"behavior": {"miniHotkey": next}}))?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ok(text: &str) -> String {
        parse(text).unwrap().canonical()
    }
    #[test]
    fn combos_come_out_canonical() {
        assert_eq!(ok("Ctrl+Alt+F"), "Ctrl+Alt+F");
        assert_eq!(ok("alt + control + f"), "Ctrl+Alt+F");
        assert_eq!(ok("Shift+Win+Ctrl+Alt+space"), "Ctrl+Alt+Shift+Win+Space");
        assert_eq!(ok("super+1"), "Win+1");
        assert_eq!(ok("Ctrl+f12"), "Ctrl+F12");
        assert_eq!(ok("Shift+F5"), "Shift+F5");
        assert_eq!(ok("Ctrl+Shift+pageup"), "Ctrl+Shift+PageUp");
        assert_eq!(ok("Ctrl+`"), "Ctrl+`");
    }
    #[test]
    fn a_modifier_is_required() {
        for text in ["F", "F5", "Shift+F", "Shift+1", "Space"] {
            assert_eq!(parse(text), Err(Refused::NeedsModifier), "{text}");
        }
    }
    #[test]
    fn nonsense_is_invalid() {
        for text in [
            "",
            "+",
            "Ctrl+",
            "Ctrl++",
            "Ctrl+Alt",
            "Ctrl+Ctrl+F",
            "Ctrl+Hyper+F",
            "Ctrl+F25",
            "Ctrl+F0",
            "Ctrl+F01",
            "Ctrl+AB",
            "Ctrl+F+G",
            "Ctrl+É",
        ] {
            assert_eq!(parse(text), Err(Refused::Invalid), "{text:?}");
        }
    }
    #[test]
    fn shortcuts_match_the_combo() {
        let s = parse("Ctrl+Shift+F").unwrap().shortcut();
        assert_eq!(
            s,
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyF)
        );
        let s = parse("Win+F24").unwrap().shortcut();
        assert_eq!(s, Shortcut::new(Some(Modifiers::SUPER), Code::F24));
    }
}
