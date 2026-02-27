pub mod config;
mod discovery;
mod manager;

pub use config::SessionConfig;
pub use discovery::SessionDiscovery;
pub use manager::SessionManager;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Idle,
    Thinking,
    Dead,
}

impl SessionStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Active => "Active",
            Self::Idle => "Idle",
            Self::Thinking => "Thinking",
            Self::Dead => "Dead",
        }
    }
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub pid: Option<u32>,
    pub cwd: PathBuf,
    pub project_name: String,
    pub git_branch: Option<String>,
    pub model: Option<String>,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub message_count: u32,
    pub tool_call_count: u32,
    pub claude_version: Option<String>,
    pub permission_mode: Option<String>,
    pub plan_slugs: Vec<String>,
    pub compaction_count: u32,
    pub hook_run_count: u32,
    pub hook_error_count: u32,
}

impl Session {
    pub fn estimated_cost_usd(&self) -> f64 {
        let model = self.model.as_deref().unwrap_or("");
        let (input_price, output_price) = model_pricing(model);
        let cache_read_price = input_price * 0.1;
        let cache_write_price = input_price * 0.25;

        (self.input_tokens as f64 * input_price
            + self.output_tokens as f64 * output_price
            + self.cache_read_tokens as f64 * cache_read_price
            + self.cache_write_tokens as f64 * cache_write_price)
            / 1_000_000.0
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_write_tokens
    }

    pub fn duration_display(&self) -> String {
        let duration = self.last_activity.signed_duration_since(self.started_at);
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m", minutes)
        }
    }

    pub fn last_activity_display(&self) -> String {
        let now = Utc::now();
        let diff = now.signed_duration_since(self.last_activity);
        let secs = diff.num_seconds();
        if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    }
}

fn model_pricing(model: &str) -> (f64, f64) {
    if model.contains("opus") {
        (15.0, 75.0)
    } else if model.contains("haiku") {
        (0.80, 4.0)
    } else {
        (3.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_session(
        model: &str,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
    ) -> Session {
        Session {
            id: "test".to_string(),
            pid: None,
            cwd: PathBuf::from("/tmp"),
            project_name: "test".to_string(),
            git_branch: None,
            model: Some(model.to_string()),
            status: SessionStatus::Dead,
            started_at: Utc::now(),
            last_activity: Utc::now(),
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
            message_count: 0,
            tool_call_count: 0,
            claude_version: None,
            permission_mode: None,
            plan_slugs: Vec::new(),
            compaction_count: 0,
            hook_run_count: 0,
            hook_error_count: 0,
        }
    }

    #[test]
    fn test_estimated_cost() {
        let session = make_session("claude-sonnet-4-20250514", 1_000_000, 100_000, 0, 0);
        let cost = session.estimated_cost_usd();
        let expected = (1_000_000.0 * 3.0 + 100_000.0 * 15.0) / 1_000_000.0;
        assert!((cost - expected).abs() < 0.001);

        let opus = make_session("claude-opus-4-20250514", 1_000_000, 100_000, 0, 0);
        let opus_cost = opus.estimated_cost_usd();
        assert!(opus_cost > cost);
    }
}
