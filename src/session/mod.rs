mod cache;
pub mod config;
mod discovery;
mod jsonl;
mod manager;
pub mod names;
pub mod pricing;
pub mod project;
pub mod registry;
mod worker;

pub use cache::CachedTranscript;
pub use discovery::{list_session_files, SessionDiscovery, SessionFile};
pub use manager::SessionManager;
pub use project::{group_projects, Project};
pub use worker::DiscoveryWorker;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_CONTEXT_PERCENT: u64 = 999;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Idle,
    Thinking,
    Waiting,
    Dead,
}

impl SessionStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Active => "Active",
            Self::Idle => "Idle",
            Self::Thinking => "Thinking",
            Self::Waiting => "Waiting",
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
    pub pid_verified: bool,
    pub session_name: Option<String>,
    pub waiting_for: Option<String>,
    pub messaging_socket: Option<String>,
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
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub context_tokens: u64,
    pub message_count: u32,
    pub tool_call_count: u32,
    pub claude_version: Option<String>,
    pub permission_mode: Option<String>,
    pub plan_slugs: Vec<String>,
    pub compaction_count: u32,
    pub hook_run_count: u32,
    pub hook_error_count: u32,
    pub api_error_count: u32,
    pub title: Option<String>,
    pub last_prompt: Option<String>,
}

impl Session {
    pub fn takeover_pid(&self) -> Option<u32> {
        self.pid.filter(|_| self.pid_verified)
    }

    pub fn estimated_cost_usd(&self) -> f64 {
        let model = self.model.as_deref().unwrap_or("");
        let p = pricing::pricing_for(model);
        (self.input_tokens as f64 * p.input
            + self.output_tokens as f64 * p.output
            + self.cache_read_tokens as f64 * p.cache_read
            + self.cache_write_5m_tokens as f64 * p.cache_write_5m
            + self.cache_write_1h_tokens as f64 * p.cache_write_1h)
            / 1_000_000.0
    }

    pub fn status_text(&self) -> String {
        match (&self.status, self.waiting_for.as_deref()) {
            (SessionStatus::Waiting, Some(detail)) => format!("Waiting: {}", detail),
            (status, _) => status.label().to_string(),
        }
    }

    pub fn cache_write_tokens(&self) -> u64 {
        self.cache_write_5m_tokens + self.cache_write_1h_tokens
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_write_tokens()
    }

    pub fn context_window(&self) -> u64 {
        pricing::context_window(self.model.as_deref().unwrap_or(""))
    }

    pub fn context_percent(&self) -> u16 {
        let window = self.context_window().max(1);
        ((self.context_tokens.saturating_mul(100)) / window).min(MAX_CONTEXT_PERCENT) as u16
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

    pub fn matches_filter(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_lowercase();
        let contains = |value: &str| value.to_lowercase().contains(&q);
        contains(&self.project_name)
            || contains(&self.cwd.to_string_lossy())
            || contains(self.git_branch.as_deref().unwrap_or(""))
            || contains(self.model.as_deref().unwrap_or(""))
            || contains(self.title.as_deref().unwrap_or(""))
            || contains(self.session_name.as_deref().unwrap_or(""))
            || contains(self.status.label())
    }
}

#[cfg(test)]
pub(crate) fn jsonl_stats_for_test(session_id: &str, message_count: u32) -> jsonl::JsonlStats {
    jsonl::JsonlStats {
        session_id: Some(session_id.to_string()),
        message_count,
        ..Default::default()
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub fn make_session(id: &str, model: &str) -> Session {
        Session {
            id: id.to_string(),
            pid: None,
            pid_verified: false,
            session_name: None,
            waiting_for: None,
            messaging_socket: None,
            cwd: PathBuf::from("/tmp"),
            project_name: "test".to_string(),
            git_branch: None,
            model: Some(model.to_string()),
            status: SessionStatus::Dead,
            started_at: Utc::now(),
            last_activity: Utc::now(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_5m_tokens: 0,
            cache_write_1h_tokens: 0,
            context_tokens: 0,
            message_count: 0,
            tool_call_count: 0,
            claude_version: None,
            permission_mode: None,
            plan_slugs: Vec::new(),
            compaction_count: 0,
            hook_run_count: 0,
            hook_error_count: 0,
            api_error_count: 0,
            title: None,
            last_prompt: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::make_session;
    use super::SessionStatus;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn test_estimated_cost_input_output() {
        let mut s = make_session("t", "claude-sonnet-4-6");
        s.input_tokens = 1_000_000;
        s.output_tokens = 100_000;
        assert!(close(s.estimated_cost_usd(), 3.0 + 1.5));

        let mut opus = make_session("t", "claude-opus-4-6");
        opus.input_tokens = 1_000_000;
        opus.output_tokens = 100_000;
        assert!(close(opus.estimated_cost_usd(), 5.0 + 2.5));
    }

    #[test]
    fn test_estimated_cost_cache_writes_cost_more_than_input() {
        let mut s = make_session("t", "claude-sonnet-4-6");
        s.cache_write_5m_tokens = 1_000_000;
        assert!(close(s.estimated_cost_usd(), 3.75));

        let mut h = make_session("t", "claude-sonnet-4-6");
        h.cache_write_1h_tokens = 1_000_000;
        assert!(close(h.estimated_cost_usd(), 6.0));

        let mut r = make_session("t", "claude-sonnet-4-6");
        r.cache_read_tokens = 1_000_000;
        assert!(close(r.estimated_cost_usd(), 0.3));
    }

    #[test]
    fn test_total_tokens_includes_both_cache_writes() {
        let mut s = make_session("t", "claude-opus-5");
        s.input_tokens = 1;
        s.output_tokens = 2;
        s.cache_read_tokens = 3;
        s.cache_write_5m_tokens = 4;
        s.cache_write_1h_tokens = 5;
        assert_eq!(s.total_tokens(), 15);
        assert_eq!(s.cache_write_tokens(), 9);
    }

    #[test]
    fn test_context_percent() {
        let mut s = make_session("t", "claude-fable-5-1");
        s.context_tokens = 300_000;
        assert_eq!(s.context_percent(), 30);
        let mut h = make_session("t", "claude-haiku-4-5");
        h.context_tokens = 150_000;
        assert_eq!(h.context_percent(), 75);
        let mut over = make_session("t", "claude-haiku-4-5");
        over.context_tokens = 600_000;
        assert_eq!(over.context_percent(), 300);
        over.context_tokens = 5_000_000;
        assert_eq!(over.context_percent(), 999);
    }

    #[test]
    fn test_status_text_includes_waiting_detail() {
        let mut s = make_session("t", "claude-opus-5");
        s.status = SessionStatus::Waiting;
        assert_eq!(s.status_text(), "Waiting");
        s.waiting_for = Some("approve Bash".into());
        assert_eq!(s.status_text(), "Waiting: approve Bash");
        s.status = SessionStatus::Idle;
        assert_eq!(s.status_text(), "Idle");
    }

    #[test]
    fn test_takeover_pid_requires_verification() {
        let mut s = make_session("t", "claude-opus-5");
        s.pid = Some(42);
        assert_eq!(s.takeover_pid(), None);
        s.pid_verified = true;
        assert_eq!(s.takeover_pid(), Some(42));
    }

    #[test]
    fn test_matches_filter_uses_title() {
        let mut s = make_session("t", "claude-opus-5");
        s.title = Some("Fix login bug".to_string());
        assert!(s.matches_filter("login"));
        assert!(!s.matches_filter("payments"));
        assert!(s.matches_filter(""));
    }
}
