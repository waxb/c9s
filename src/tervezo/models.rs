use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationStatus {
    Pending,
    Queued,
    Running,
    Completed,
    Merged,
    Failed,
    Stopped,
    Cancelled,
}

impl ImplementationStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Pending => "Pending",
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Completed => "Done",
            Self::Merged => "Merged",
            Self::Failed => "Failed",
            Self::Stopped => "Stopped",
            Self::Cancelled => "Cancelled",
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    #[allow(dead_code)]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Merged | Self::Failed | Self::Stopped | Self::Cancelled
        )
    }
}

impl std::fmt::Display for ImplementationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub status: ImplementationStatus,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default, alias = "repository")]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub estimated_cost_usd: Option<f64>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
    #[serde(default)]
    pub message_count: Option<u32>,
    #[serde(default)]
    pub pr_url: Option<String>,
    #[serde(default)]
    pub pr_number: Option<u32>,
    #[serde(default)]
    pub pr_status: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

impl Implementation {
    pub fn display_name(&self) -> &str {
        self.title.as_deref().unwrap_or("(untitled)")
    }

    #[allow(dead_code)]
    pub fn repository(&self) -> Option<&str> {
        self.repo_url.as_deref()
    }

    pub fn last_activity_display(&self) -> String {
        let ts = self.updated_at.or(self.created_at);
        match ts {
            Some(dt) => {
                let now = Utc::now();
                let diff = now.signed_duration_since(dt);
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
            None => "-".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn repo_short(&self) -> String {
        match &self.repo_url {
            Some(r) => r.rsplit('/').next().unwrap_or(r).to_string(),
            None => "-".to_string(),
        }
    }
}

/// Timeline messages have loosely-typed schema (`anyOf: [{}, null]`).
/// Actual observed fields: id, type, reason, timestamp, toStatus, fromStatus,
/// plus varying fields per type (e.g. content, message, output for other types).
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineMessage {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<DateTime<Utc>>,
    /// Message type: "status_change", "agent_message", "tool_call", "test_report", etc.
    #[serde(default, rename = "type")]
    pub msg_type: Option<String>,
    /// Used by status_change messages as display text.
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub to_status: Option<String>,
    #[serde(default)]
    pub from_status: Option<String>,
    /// Used by assistant_text messages.
    #[serde(default)]
    pub text: Option<String>,
    /// Used by agent_message / tool_call messages.
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    /// Generic status field (if present).
    #[serde(default)]
    pub status: Option<String>,
}

impl TimelineMessage {
    /// Best-effort display text: check all known text fields in priority order.
    pub fn display_text(&self) -> &str {
        self.reason
            .as_deref()
            .or(self.text.as_deref())
            .or(self.message.as_deref())
            .or(self.content.as_deref())
            .or(self.output.as_deref())
            .or(self.details.as_deref())
            .unwrap_or("")
    }

    /// Status for icon rendering: prefer to_status, fall back to status.
    pub fn effective_status(&self) -> Option<&str> {
        self.to_status.as_deref().or(self.status.as_deref())
    }
}

/// File changes are loosely typed in the spec (`anyOf: [{}, null]`).
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    #[serde(default, alias = "filename")]
    pub path: Option<String>,
    #[serde(default, alias = "patch")]
    pub diff: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub additions: Option<u32>,
    #[serde(default)]
    pub deletions: Option<u32>,
    #[serde(default)]
    pub changes: Option<u32>,
}

impl FileChange {
    pub fn display_path(&self) -> &str {
        self.path.as_deref().unwrap_or("(unknown)")
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshCredentials {
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    pub ssh_command: String,
    #[serde(default)]
    pub sandbox_id: Option<String>,
    #[serde(default)]
    pub sandbox_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub order: Option<u32>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub timeline_message_count: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ListResponse {
    pub items: Vec<Implementation>,
    #[serde(default)]
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineResponse {
    pub messages: Vec<Option<TimelineMessage>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct StepsResponse {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChangesResponse {
    pub files: Vec<Option<FileChange>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanResponse {
    pub plan: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisResponse {
    pub analysis: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestOutputResponse {
    #[serde(default)]
    pub test_reports: Vec<serde_json::Value>,
}
