use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    /// Truncated to 200 chars — the timeline shows one line per message.
    pub fn display_text(&self) -> String {
        let raw = self.display_text_raw();
        truncate_display(&raw, 200)
    }

    fn display_text_raw(&self) -> String {
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
                    let end = val
                        .char_indices()
                        .map(|(i, _)| i)
                        .take_while(|&i| i <= 77)
                        .last()
                        .unwrap_or(0);
                    format!("{}...", &val[..end])
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
                    let end = s
                        .char_indices()
                        .map(|(i, _)| i)
                        .take_while(|&i| i <= 57)
                        .last()
                        .unwrap_or(0);
                    format!("{}...", &s[..end])
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
                            let end = s
                                .char_indices()
                                .map(|(i, _)| i)
                                .take_while(|&i| i <= 49)
                                .last()
                                .unwrap_or(0);
                            format!("{}…", &s[..end])
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

fn truncate_display(s: &str, max: usize) -> String {
    // Take only the first line to avoid multi-line display text
    let first_line = s.lines().next().unwrap_or("");
    if first_line.len() <= max {
        return first_line.to_string();
    }
    let end = first_line
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max.saturating_sub(3))
        .last()
        .unwrap_or(0);
    format!("{}...", &first_line[..end])
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
    pub critical_path: Option<String>,
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

// --- Status / Steps API response ---

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub status: String,
    #[serde(default)]
    pub waiting_for_input: bool,
    #[serde(default)]
    pub current_step_name: Option<String>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub steps: Vec<StatusStep>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusStep {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub error: Option<String>,
}

// --- PR details ---

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDetails {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub number: Option<u32>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub merged: bool,
    #[serde(default)]
    pub draft: bool,
}

impl PrDetails {
    pub fn is_open(&self) -> bool {
        self.status.as_deref() == Some("open")
    }

    pub fn is_closed(&self) -> bool {
        self.status.as_deref() == Some("closed")
    }
}

// --- POST action response types ---

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePrResponse {
    #[serde(default)]
    pub pr_url: Option<String>,
    #[serde(default)]
    pub pr_number: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SuccessResponse {
    #[serde(default)]
    pub success: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartResponse {
    #[serde(default)]
    pub implementation_id: Option<String>,
    #[serde(default)]
    pub is_new_implementation: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptResponse {
    #[serde(default)]
    pub sent: bool,
    #[serde(default)]
    pub follow_up_id: Option<String>,
}

// --- Prompt request body ---

#[derive(Debug, Serialize)]
pub struct PromptRequest {
    pub message: String,
}

// --- Workspace ---

#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspacesResponse {
    pub items: Vec<Workspace>,
}

// --- Create implementation request body ---

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateImplementationRequest {
    pub prompt: String,
    pub mode: String,
    pub workspace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
}

// --- Duration formatting ---

pub fn format_duration_secs(secs: f64) -> String {
    if secs < 1.0 {
        "< 1s".to_string()
    } else if secs < 60.0 {
        format!("{}s", secs as u64)
    } else if secs < 3600.0 {
        let m = (secs / 60.0) as u64;
        let s = (secs % 60.0) as u64;
        if s == 0 {
            format!("{}m", m)
        } else {
            format!("{}m {}s", m, s)
        }
    } else {
        let h = (secs / 3600.0) as u64;
        let m = ((secs % 3600.0) / 60.0) as u64;
        if m == 0 {
            format!("{}h", h)
        } else {
            format!("{}h {}m", h, m)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_implementation_request_serialization_camel_case() {
        let req = CreateImplementationRequest {
            prompt: "Fix login bug".to_string(),
            mode: "bug_fix".to_string(),
            workspace_id: "ws-123".to_string(),
            repository_name: Some("user/repo".to_string()),
            base_branch: Some("develop".to_string()),
        };
        let json: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(json["prompt"], "Fix login bug");
        assert_eq!(json["mode"], "bug_fix");
        assert_eq!(json["workspaceId"], "ws-123");
        assert_eq!(json["repositoryName"], "user/repo");
        assert_eq!(json["baseBranch"], "develop");
        assert!(json.get("workspace_id").is_none());
        assert!(json.get("repository_name").is_none());
        assert!(json.get("base_branch").is_none());
    }

    #[test]
    fn test_create_implementation_request_omits_null_optional_fields() {
        let req = CreateImplementationRequest {
            prompt: "Add feature".to_string(),
            mode: "feature".to_string(),
            workspace_id: "ws-456".to_string(),
            repository_name: None,
            base_branch: None,
        };
        let json: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert!(json.get("baseBranch").is_none());
        assert!(json.get("repositoryName").is_none());
        assert_eq!(json["workspaceId"], "ws-456");
    }

    #[test]
    fn test_added_deserializes_critical_path_string() {
        let json = r#"{"file": "test.rs", "count": 3, "criticalPath": "true"}"#;
        let ta: TestAdded = serde_json::from_str(json).unwrap();
        assert_eq!(ta.critical_path, Some("true".to_string()));
        assert_eq!(ta.file, Some("test.rs".to_string()));
        assert_eq!(ta.count, Some(3));
    }

    #[test]
    fn test_added_deserializes_without_critical_path() {
        let json = r#"{"file": "test.rs", "count": 1}"#;
        let ta: TestAdded = serde_json::from_str(json).unwrap();
        assert_eq!(ta.critical_path, None);
    }

    #[test]
    fn test_report_deserializes_with_string_critical_path() {
        let json = r#"{
            "summary": {"totalTests": 5, "passRate": "80%"},
            "testsAdded": [
                {"file": "auth.rs", "count": 2, "criticalPath": "true"},
                {"file": "utils.rs", "count": 1, "criticalPath": "false"}
            ],
            "uncoveredPaths": []
        }"#;
        let report: TestReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.tests_added.len(), 2);
        assert_eq!(report.tests_added[0].critical_path.as_deref(), Some("true"));
        assert_eq!(
            report.tests_added[1].critical_path.as_deref(),
            Some("false")
        );
    }

    #[test]
    fn test_is_running_true_for_running_status() {
        assert!(ImplementationStatus::Running.is_running());
    }

    #[test]
    fn test_is_running_false_for_non_running_statuses() {
        let non_running = [
            ImplementationStatus::Pending,
            ImplementationStatus::Queued,
            ImplementationStatus::Completed,
            ImplementationStatus::Merged,
            ImplementationStatus::Failed,
            ImplementationStatus::Stopped,
            ImplementationStatus::Cancelled,
        ];
        for status in &non_running {
            assert!(!status.is_running(), "{:?} should not be running", status);
        }
    }

    #[test]
    fn test_is_terminal_for_terminal_statuses() {
        let terminal = [
            ImplementationStatus::Completed,
            ImplementationStatus::Merged,
            ImplementationStatus::Failed,
            ImplementationStatus::Stopped,
            ImplementationStatus::Cancelled,
        ];
        for status in &terminal {
            assert!(status.is_terminal(), "{:?} should be terminal", status);
        }
    }

    #[test]
    fn test_is_terminal_false_for_active_statuses() {
        let active = [
            ImplementationStatus::Pending,
            ImplementationStatus::Queued,
            ImplementationStatus::Running,
        ];
        for status in &active {
            assert!(!status.is_terminal(), "{:?} should not be terminal", status);
        }
    }

    #[test]
    fn test_status_deserialization() {
        let json = r#""running""#;
        let status: ImplementationStatus = serde_json::from_str(json).unwrap();
        assert!(status.is_running());

        let json = r#""completed""#;
        let status: ImplementationStatus = serde_json::from_str(json).unwrap();
        assert!(status.is_terminal());
        assert!(!status.is_running());
    }

    #[test]
    fn test_status_label() {
        assert_eq!(ImplementationStatus::Running.label(), "Running");
        assert_eq!(ImplementationStatus::Completed.label(), "Done");
        assert_eq!(ImplementationStatus::Failed.label(), "Failed");
    }

    #[test]
    fn test_implementation_display_name_fallback() {
        let impl_ = Implementation {
            id: "test-id".to_string(),
            title: None,
            status: ImplementationStatus::Running,
            branch: None,
            repo_url: None,
            created_at: None,
            updated_at: None,
            estimated_cost_usd: None,
            total_tokens: None,
            message_count: None,
            pr_url: None,
            pr_number: None,
            pr_status: None,
            mode: None,
        };
        assert_eq!(impl_.display_name(), "(untitled)");
    }

    #[test]
    fn test_implementation_display_name_with_title() {
        let impl_ = Implementation {
            id: "test-id".to_string(),
            title: Some("Fix bug".to_string()),
            status: ImplementationStatus::Running,
            branch: None,
            repo_url: None,
            created_at: None,
            updated_at: None,
            estimated_cost_usd: None,
            total_tokens: None,
            message_count: None,
            pr_url: None,
            pr_number: None,
            pr_status: None,
            mode: None,
        };
        assert_eq!(impl_.display_name(), "Fix bug");
    }

    #[test]
    fn test_file_change_display_path_fallback() {
        let fc = FileChange {
            path: None,
            diff: None,
            status: None,
            additions: None,
            deletions: None,
            changes: None,
        };
        assert_eq!(fc.display_path(), "(unknown)");
    }

    #[test]
    fn test_pr_details_status() {
        let pr = PrDetails {
            url: None,
            number: None,
            status: Some("open".to_string()),
            title: None,
            mergeable: None,
            merged: false,
            draft: false,
        };
        assert!(pr.is_open());
        assert!(!pr.is_closed());

        let pr_closed = PrDetails {
            url: None,
            number: None,
            status: Some("closed".to_string()),
            title: None,
            mergeable: None,
            merged: false,
            draft: false,
        };
        assert!(!pr_closed.is_open());
        assert!(pr_closed.is_closed());
    }

    #[test]
    fn test_format_duration_secs() {
        assert_eq!(format_duration_secs(0.5), "< 1s");
        assert_eq!(format_duration_secs(45.0), "45s");
        assert_eq!(format_duration_secs(120.0), "2m");
        assert_eq!(format_duration_secs(125.0), "2m 5s");
        assert_eq!(format_duration_secs(3600.0), "1h");
        assert_eq!(format_duration_secs(3720.0), "1h 2m");
    }

    #[test]
    fn test_truncate_display_short_string() {
        assert_eq!(truncate_display("hello", 200), "hello");
    }

    #[test]
    fn test_truncate_display_exact_limit() {
        let s = "a".repeat(200);
        assert_eq!(truncate_display(&s, 200), s);
    }

    #[test]
    fn test_truncate_display_long_string_is_truncated() {
        let s = "a".repeat(300);
        let result = truncate_display(&s, 200);
        assert!(result.len() <= 203);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_display_multiline_takes_first_line() {
        let s = "first line\nsecond line\nthird line";
        assert_eq!(truncate_display(s, 200), "first line");
    }

    #[test]
    fn test_truncate_display_multiline_long_first_line() {
        let long_first = "a".repeat(300);
        let s = format!("{}\nsecond line", long_first);
        let result = truncate_display(&s, 200);
        assert!(result.ends_with("..."));
        assert!(!result.contains('\n'));
    }

    #[test]
    fn test_truncate_display_empty_string() {
        assert_eq!(truncate_display("", 200), "");
    }

    #[test]
    fn test_truncate_display_unicode_safe() {
        let s = "a".repeat(198) + "\u{00e9}\u{00e9}";
        let result = truncate_display(&s, 200);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_display_text_long_message_truncated() {
        let long_text = "x".repeat(500);
        let msg = TimelineMessage {
            text: Some(long_text),
            ..default_timeline_message()
        };
        let result = msg.display_text();
        assert!(result.len() <= 203);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_display_text_multiline_message_single_line() {
        let multiline =
            "Line one of a very long message\nLine two continues here\nLine three also present";
        let msg = TimelineMessage {
            text: Some(multiline.to_string()),
            ..default_timeline_message()
        };
        let result = msg.display_text();
        assert!(!result.contains('\n'));
        assert_eq!(result, "Line one of a very long message");
    }

    #[test]
    fn test_display_text_reason_field_truncated() {
        let long_reason = "r".repeat(500);
        let msg = TimelineMessage {
            reason: Some(long_reason),
            ..default_timeline_message()
        };
        let result = msg.display_text();
        assert!(result.len() <= 203);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_display_text_short_message_not_truncated() {
        let msg = TimelineMessage {
            text: Some("Hello world".to_string()),
            ..default_timeline_message()
        };
        assert_eq!(msg.display_text(), "Hello world");
    }

    #[test]
    fn test_display_text_empty_message() {
        let msg = default_timeline_message();
        assert_eq!(msg.display_text(), "");
    }

    #[test]
    fn test_has_inline_code_with_diff() {
        let msg = TimelineMessage {
            msg_type: Some("file_change".to_string()),
            diff: Some("--- a/file\n+++ b/file\n+new line".to_string()),
            ..default_timeline_message()
        };
        assert!(msg.has_inline_code());
    }

    #[test]
    fn test_has_inline_code_without_file_change() {
        let msg = TimelineMessage {
            msg_type: Some("assistant_text".to_string()),
            diff: Some("some diff".to_string()),
            ..default_timeline_message()
        };
        assert!(!msg.has_inline_code());
    }

    #[test]
    fn test_timeline_timeout_is_60_seconds() {
        use super::super::api::TIMELINE_TIMEOUT_SECS;
        assert_eq!(TIMELINE_TIMEOUT_SECS, 60);
        // Must be significantly larger than the default request timeout (10s)
        assert!(TIMELINE_TIMEOUT_SECS > 10);
    }

    #[test]
    fn test_tervezo_detail_state_timeline_error_default_none() {
        let imp = Implementation {
            id: "test-id".to_string(),
            title: Some("Test".to_string()),
            status: ImplementationStatus::Running,
            branch: None,
            repo_url: None,
            created_at: None,
            updated_at: None,
            estimated_cost_usd: None,
            total_tokens: None,
            message_count: None,
            pr_url: None,
            pr_number: None,
            pr_status: None,
            mode: None,
        };
        let state = crate::app::TervezoDetailState::new(imp);
        assert!(state.timeline_error.is_none());
    }

    fn default_timeline_message() -> TimelineMessage {
        TimelineMessage {
            id: None,
            timestamp: None,
            msg_type: None,
            reason: None,
            to_status: None,
            from_status: None,
            text: None,
            message: None,
            content: None,
            output: None,
            details: None,
            status: None,
            tool_name: None,
            parameters: None,
            file_path: None,
            diff: None,
            thinking: None,
            thought: None,
            summary: None,
            title: None,
            todos: None,
            event: None,
            iteration: None,
            tool_call_id: None,
            is_error: None,
            operation: None,
            success: None,
            branch: None,
            commit_hash: None,
            commit_message: None,
            pr_url: None,
            pr_number: None,
            severity: None,
            is_partial: None,
            tests_added: None,
            approach: None,
            uncovered_paths: None,
        }
    }
}
