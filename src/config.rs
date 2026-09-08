use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use std::path::PathBuf;

use crate::tlog;

const DEFAULT_SESSION_PANE_KEY: &str = "ctrl-]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    key: char,
}

fn ctrl_alias(key: char) -> Option<char> {
    match key {
        '\\' => Some('4'),
        ']' => Some('5'),
        '^' => Some('6'),
        '_' => Some('7'),
        _ => None,
    }
}

impl KeyBinding {
    pub fn parse(spec: &str) -> Result<Self, String> {
        let lower = spec.trim().to_ascii_lowercase();
        let rest = ["ctrl-", "ctrl+", "c-"]
            .iter()
            .find_map(|prefix| lower.strip_prefix(prefix))
            .ok_or_else(|| format!("key binding {:?} must be ctrl-<char>", spec))?;
        let mut chars = rest.chars();
        match (chars.next(), chars.next()) {
            (Some(key), None) if key.is_ascii_graphic() => Ok(Self { key }),
            _ => Err(format!("key binding {:?} must name one character", spec)),
        }
    }

    pub fn matches(&self, key: &KeyEvent) -> bool {
        if !key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match key.code {
            KeyCode::Char(c) => c == self.key || Some(c) == ctrl_alias(self.key),
            _ => false,
        }
    }

    pub fn label(&self) -> String {
        format!("Ctrl+{}", self.key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    pub session_pane: KeyBinding,
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            session_pane: KeyBinding::parse(DEFAULT_SESSION_PANE_KEY)
                .expect("default key binding is valid"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    pub keys: Keys,
}

#[derive(Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    keys: RawKeys,
}

#[derive(Deserialize, Default)]
struct RawKeys {
    session_pane: Option<String>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
        let session_pane = match raw.keys.session_pane {
            Some(spec) => KeyBinding::parse(&spec)?,
            None => Keys::default().session_pane,
        };
        Ok(Self {
            keys: Keys { session_pane },
        })
    }

    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match Self::parse(&text) {
            Ok(config) => config,
            Err(e) => {
                tlog!(warn, "config {}: {} (using defaults)", path.display(), e);
                Self::default()
            }
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".c9s").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_default_binding_matches_crossterm_and_kitty_forms() {
        let keys = Keys::default();
        assert!(keys.session_pane.matches(&ctrl('5')));
        assert!(keys.session_pane.matches(&ctrl(']')));
        assert!(!keys.session_pane.matches(&ctrl('y')));
        let mut plain = ctrl(']');
        plain.modifiers = KeyModifiers::NONE;
        assert!(!keys.session_pane.matches(&plain));
        assert_eq!(keys.session_pane.label(), "Ctrl+]");
    }

    #[test]
    fn test_parse_variants_and_errors() {
        assert_eq!(KeyBinding::parse("ctrl-y"), Ok(KeyBinding { key: 'y' }));
        assert_eq!(KeyBinding::parse("Ctrl+Y"), Ok(KeyBinding { key: 'y' }));
        assert_eq!(KeyBinding::parse("C-]"), Ok(KeyBinding { key: ']' }));
        assert!(KeyBinding::parse("y").is_err());
        assert!(KeyBinding::parse("ctrl-").is_err());
        assert!(KeyBinding::parse("ctrl-ab").is_err());
    }

    #[test]
    fn test_config_parse_ignores_unknown_sections() {
        let text = "[tervezo]\napi_key = \"x\"\n\n[keys]\nsession_pane = \"ctrl-y\"\n";
        let config = Config::parse(text).unwrap();
        assert!(config.keys.session_pane.matches(&ctrl('y')));
        assert_eq!(Config::parse("").unwrap(), Config::default());
        assert!(Config::parse("[keys]\nsession_pane = \"nope\"\n").is_err());
    }
}
