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
    #[serde(default, alias = "timelineMessageCount")]
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
    /// Tool call fields.
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
    /// File change fields.
    #[serde(default, alias = "filePath", alias = "filename")]
    pub file_path: Option<String>,
    #[serde(default)]
    pub diff: Option<String>,
    /// Thinking fields — exact name TBD, capture common variants.
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub thought: Option<String>,
    #[serde(default)]
    pub summary: Option<serde_json::Value>,
    /// Title used by some message types.
    #[serde(default)]
    pub title: Option<String>,
    /// Todo/task list fields.
    #[serde(default)]
    pub todos: Option<Vec<serde_json::Value>>,
    /// Iteration marker fields.
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub iteration: Option<u32>,
    /// Tool call correlation.
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Error flag on tool_result.
    #[serde(default)]
    pub is_error: Option<bool>,
    /// Operation type for file_change ("create"/"edit"/"delete") and git_operation.
    #[serde(default)]
    pub operation: Option<String>,
    /// Success flag for git_operation.
    #[serde(default)]
    pub success: Option<bool>,
    /// Git operation fields.
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commit_hash: Option<String>,
    #[serde(default)]
    pub commit_message: Option<String>,
    /// PR created fields.
    #[serde(default)]
    pub pr_url: Option<String>,
    #[serde(default)]
    pub pr_number: Option<u32>,
    /// Error severity ("warning"/"error"/"fatal").
    #[serde(default)]
    pub severity: Option<String>,
    /// Partial flag for assistant_text.
    #[serde(default)]
    pub is_partial: Option<bool>,
    /// Test report fields (on test_report messages).
    #[serde(default)]
    pub tests_added: Option<Vec<TestAdded>>,
    #[serde(default)]
    pub approach: Option<String>,
    #[serde(default)]
    pub uncovered_paths: Option<Vec<UncoveredPath>>,
}

impl TimelineMessage {
    /// Best-effort display text: check all known text fields in priority order.
    /// Returns owned string because tool_call messages need composing.
    pub fn display_text(&self) -> String {
        // status_change: use reason
        if let Some(ref reason) = self.reason {
            return reason.clone();
        }

        // tool_call: compose "ToolName arg"
        if let Some(ref tool) = self.tool_name {
            let arg = self.tool_call_summary();
            return if arg.is_empty() {
                tool.clone()
            } else {
                format!("{} {}", tool, arg)
            };
        }

        // file_change: show file_path or first line of diff
        if self.msg_type.as_deref() == Some("file_change") {
            let op = self.operation.as_deref().unwrap_or("Changed");
            let op_label = match op {
                "create" => "Created",
                "delete" => "Deleted",
                "edit" => "Changed",
                _ => "Changed",
            };
            if let Some(ref fpath) = self.file_path {
                return format!("{} {}", op_label, fpath);
            }
            if let Some(ref d) = self.diff {
                return self.diff_summary(d);
            }
            if self.content.is_some() {
                return "New file".to_string();
            }
        }

        // pr_created: show PR URL/number
        if self.msg_type.as_deref() == Some("pr_created") {
            if let Some(ref url) = self.pr_url {
                return format!("PR created: {}", url);
            }
            if let Some(num) = self.pr_number {
                return format!("PR #{} created", num);
            }
            return "PR created".to_string();
        }

        // git_operation: show operation + branch/commit
        if self.msg_type.as_deref() == Some("git_operation") {
            let op = self.operation.as_deref().unwrap_or("git");
            let detail = self
                .commit_message
                .as_deref()
                .or(self.branch.as_deref())
                .or(self.commit_hash.as_deref())
                .unwrap_or("");
            let status_icon = if self.success == Some(true) {
                "ok"
            } else if self.success == Some(false) {
                "failed"
            } else {
                ""
            };
            return if detail.is_empty() {
                format!("{} {}", op, status_icon).trim().to_string()
            } else {
                format!("{} {} {}", op, detail, status_icon)
                    .trim()
                    .to_string()
            };
        }

        // error: show severity + message
        if self.msg_type.as_deref() == Some("error") {
            let sev = self.severity.as_deref().unwrap_or("error");
            let msg = self
                .message
                .as_deref()
                .or(self.content.as_deref())
                .or(self.details.as_deref())
                .unwrap_or("Unknown error");
            return format!("[{}] {}", sev, msg);
        }

        // todo: show active tasks
        if self.msg_type.as_deref() == Some("todo") {
            return self.todo_summary();
        }

        // iteration_marker: show iteration boundary
        if self.msg_type.as_deref() == Some("iteration_marker") {
            let event = self.event.as_deref().unwrap_or("marker");
            let iter_num = self.iteration.unwrap_or(0);
            return match event {
                "start" => format!("── Iteration {} started ──", iter_num),
                "complete" => format!("── Iteration {} complete ──", iter_num),
                _ => format!("── Iteration {} ──", iter_num),
            };
        }

        // assistant_thinking: check thinking-related fields
        if matches!(
            self.msg_type.as_deref(),
            Some("thinking") | Some("assistant_thinking")
        ) {
            if let Some(t) = self
                .thinking
                .as_deref()
                .or(self.thought.as_deref())
                .or(self.summary_as_str())
                .or(self.title.as_deref())
                .or(self.content.as_deref())
                .or(self.text.as_deref())
                .or(self.message.as_deref())
            {
                return t.to_string();
            }
            return "Thinking...".to_string();
        }

        // assistant_text / agent_message / generic
        self.text
            .as_deref()
            .or(self.message.as_deref())
            .or(self.content.as_deref())
            .or(self.output.as_deref())
            .or(self.details.as_deref())
            .or(self.summary_as_str())
            .or(self.thinking.as_deref())
            .or(self.title.as_deref())
            .unwrap_or("")
            .to_string()
    }

    /// Extract the most relevant parameter from a tool_call for display.
    fn tool_call_summary(&self) -> String {
        let params = match &self.parameters {
            Some(v) => v,
            None => return String::new(),
        };
        let obj = match params.as_object() {
            Some(o) => o,
            None => return String::new(),
        };

        // Priority: file_path > path > pattern > command > query > glob > first string value
        for key in &["file_path", "path", "pattern", "command", "query", "glob"] {
            if let Some(val) = obj.get(*key).and_then(|v| v.as_str()) {
                let truncated = if val.len() > 80 {
                    format!("{}...", &val[..77])
                } else {
                    val.to_string()
                };
                return truncated;
            }
        }

        // Fallback: first string value
        for val in obj.values() {
            if let Some(s) = val.as_str() {
                let truncated = if s.len() > 60 {
                    format!("{}...", &s[..57])
                } else {
                    s.to_string()
                };
                return truncated;
            }
        }

        String::new()
    }

    /// Extract a one-line summary from a unified diff.
    fn diff_summary(&self, diff: &str) -> String {
        // Try to find the file path from --- or +++ headers
        for line in diff.lines() {
            if let Some(path) = line.strip_prefix("+++ b/") {
                return format!("Changed {}", path);
            }
            if let Some(path) = line.strip_prefix("+++ ") {
                if path != "/dev/null" {
                    return format!("Changed {}", path);
                }
            }
            if let Some(path) = line.strip_prefix("--- a/") {
                // Will be overridden by +++ if present, but use as fallback
                if !diff.contains("+++ ") {
                    return format!("Removed {}", path);
                }
            }
        }
        // Count additions/deletions as summary
        let adds = diff
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .count();
        let dels = diff
            .lines()
            .filter(|l| l.starts_with('-') && !l.starts_with("---"))
            .count();
        format!("File changed (+{} -{})", adds, dels)
    }

    /// Extract a string from the `summary` field, which can be either a JSON string
    /// or an object (for test_report messages).
    fn summary_as_str(&self) -> Option<&str> {
        self.summary.as_ref().and_then(|v| v.as_str())
    }

    /// Whether this message has inline code to render (diff or new file content).
    pub fn has_inline_code(&self) -> bool {
        self.msg_type.as_deref() == Some("file_change")
            && (self.diff.is_some() || self.content.is_some())
    }

    /// Summarize todos into a one-line description.
    fn todo_summary(&self) -> String {
        let todos = match &self.todos {
            Some(t) => t,
            None => return "Tasks updated".to_string(),
        };
        let items: Vec<String> = todos
            .iter()
            .filter_map(|t| {
                t.get("activeForm")
                    .and_then(|v| v.as_str())
                    .or_else(|| t.get("content").and_then(|v| v.as_str()))
                    .map(|s| {
                        if s.len() > 50 {
                            format!("{}…", &s[..49])
                        } else {
                            s.to_string()
                        }
                    })
            })
            .collect();
        if items.is_empty() {
            "Tasks updated".to_string()
        } else if items.len() == 1 {
            items[0].clone()
        } else {
            format!("{} (+{} more)", items[0], items.len() - 1)
        }
    }

    /// Status for icon rendering: prefer to_status, fall back to status.
    pub fn effective_status(&self) -> Option<&str> {
        self.to_status.as_deref().or(self.status.as_deref())
    }

    /// Whether this message is a tool call.
    #[allow(dead_code)]
    pub fn is_tool_call(&self) -> bool {
        self.msg_type.as_deref() == Some("tool_call")
    }

    /// Whether this is assistant text.
    #[allow(dead_code)]
    pub fn is_assistant_text(&self) -> bool {
        self.msg_type.as_deref() == Some("assistant_text")
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

#[allow(dead_code)]
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

// --- Typed test report structs ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestReportSummary {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub stats: Option<TestReportStats>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestReportStats {
    #[serde(default)]
    pub new_tests: Option<u32>,
    #[serde(default)]
    pub total_before: Option<u32>,
    #[serde(default)]
    pub total_after: Option<u32>,
    #[serde(default)]
    pub pre_existing_failures: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAdded {
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub critical_path: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UncoveredPath {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub verification_method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestReport {
    #[serde(default)]
    pub summary: Option<TestReportSummary>,
    #[serde(default)]
    pub tests_added: Vec<TestAdded>,
    #[serde(default)]
    pub approach: Option<String>,
    #[serde(default)]
    pub uncovered_paths: Vec<UncoveredPath>,
}
