use crate::tlog;

use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_BASE_URL: &str = "https://app.tervezo.ai/api/v1";
const DEFAULT_POLL_INTERVAL: u64 = 30;

#[derive(Debug, Clone)]
pub struct TervezoConfig {
    pub api_key: String,
    pub base_url: String,
    pub poll_interval: u64,
}

#[derive(Deserialize)]
struct ConfigFile {
    tervezo: Option<TervezoSection>,
}

#[derive(Deserialize)]
struct TervezoSection {
    api_key: Option<String>,
    base_url: Option<String>,
    poll_interval: Option<u64>,
}

impl TervezoConfig {
    pub fn load() -> Option<Self> {
        let api_key = Self::resolve_api_key()?;

        let (base_url, poll_interval) = Self::read_file_settings();

        let config = Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            poll_interval: poll_interval.unwrap_or(DEFAULT_POLL_INTERVAL),
        };

        tlog!(
            info,
            "config loaded: base_url={} poll={}s key={}...{}",
            config.base_url,
            config.poll_interval,
            &config.api_key[..6.min(config.api_key.len())],
            &config.api_key[config.api_key.len().saturating_sub(4)..],
        );

        Some(config)
    }

    fn resolve_api_key() -> Option<String> {
        if let Ok(key) = std::env::var("TERVEZO_API_KEY") {
            if !key.is_empty() {
                return Some(key);
            }
        }

        let section = Self::read_config_file()?;
        section.api_key.filter(|k| !k.is_empty())
    }

    fn read_file_settings() -> (Option<String>, Option<u64>) {
        match Self::read_config_file() {
            Some(section) => (section.base_url, section.poll_interval),
            None => (None, None),
        }
    }

    fn read_config_file() -> Option<TervezoSection> {
        let path = config_path()?;
        let content = std::fs::read_to_string(path).ok()?;
        let file: ConfigFile = toml::from_str(&content).ok()?;
        file.tervezo
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".c9s").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path() {
        let path = config_path();
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.ends_with(".c9s/config.toml"));
    }
}
