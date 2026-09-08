use serde_json::Value;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TOOL_WAIT_MS: u64 = 5000;

#[derive(Clone, Copy, PartialEq)]
enum SessionState {
    Unknown,
    UserSent,
    Working,
    Idle,
    ToolWait,
}

pub struct JsonlNotifier {
    jsonl_path: PathBuf,
    last_known_size: u64,
    read_offset: u64,
    state: SessionState,
    tool_use_at: Option<u64>,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl JsonlNotifier {
    pub fn new(cwd: &Path, session_id: &str) -> Self {
        let claude_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".claude");

        let encoded = cwd.to_string_lossy().replace('/', "-");
        let project_dir = claude_dir.join("projects").join(&encoded);

        let jsonl_path = project_dir.join(format!("{}.jsonl", session_id));
        let size = std::fs::metadata(&jsonl_path).map(|m| m.len()).unwrap_or(0);

        Self {
            jsonl_path,
            last_known_size: size,
            read_offset: size,
            state: SessionState::Unknown,
            tool_use_at: None,
        }
    }

    pub fn check(&mut self) -> bool {
        let path = self.jsonl_path.clone();
        let current_size = match std::fs::metadata(&path) {
            Ok(m) => m.len(),
            Err(_) => return false,
        };

        if current_size == self.last_known_size && self.read_offset >= self.last_known_size {
            return self.tool_wait_expired();
        }

        if current_size < self.read_offset {
            self.last_known_size = current_size;
            self.read_offset = current_size;
            return false;
        }

        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        let mut reader = BufReader::new(file);
        if reader.seek(SeekFrom::Start(self.read_offset)).is_err() {
            return false;
        }

        let mut should_notify = false;
        let mut line = String::new();
        let mut good_offset = self.read_offset;

        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            if !line.ends_with('\n') {
                break;
            }
            good_offset += line.len() as u64;
            let trimmed = line.trim();
            if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                should_notify |= self.apply_line(&value);
            }
            line.clear();
        }

        self.last_known_size = current_size;
        self.read_offset = good_offset;
        should_notify
    }

    fn tool_wait_expired(&mut self) -> bool {
        let Some(tool_at) = self.tool_use_at else {
            return false;
        };
        if now_millis().saturating_sub(tool_at) >= TOOL_WAIT_MS {
            self.tool_use_at = None;
            return true;
        }
        false
    }

    fn in_progress(&self) -> bool {
        matches!(
            self.state,
            SessionState::UserSent | SessionState::Working | SessionState::ToolWait
        )
    }

    fn apply_line(&mut self, value: &Value) -> bool {
        if value.get("isCompactSummary").and_then(|v| v.as_bool()) == Some(true) {
            self.state = SessionState::UserSent;
            self.tool_use_at = None;
            return false;
        }

        let msg_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match msg_type {
            "user" => {
                self.state = SessionState::UserSent;
                self.tool_use_at = None;
                false
            }
            "assistant" => {
                let stop_reason = value
                    .get("message")
                    .and_then(|m| m.get("stop_reason"))
                    .and_then(|s| s.as_str());
                self.apply_stop_reason(stop_reason)
            }
            "system" => {
                let subtype = value.get("subtype").and_then(|v| v.as_str());
                if matches!(subtype, Some("stop_hook_summary" | "turn_duration")) {
                    let notify = self.in_progress();
                    self.state = SessionState::Idle;
                    self.tool_use_at = None;
                    return notify;
                }
                false
            }
            "progress" if self.state != SessionState::ToolWait => {
                self.state = SessionState::Working;
                false
            }
            _ => false,
        }
    }

    fn apply_stop_reason(&mut self, stop_reason: Option<&str>) -> bool {
        match stop_reason {
            Some("end_turn") => {
                let notify = self.in_progress();
                self.state = SessionState::Idle;
                self.tool_use_at = None;
                notify
            }
            Some("tool_use") => {
                self.state = SessionState::ToolWait;
                self.tool_use_at = Some(now_millis());
                false
            }
            _ => {
                if self.state != SessionState::ToolWait {
                    self.state = SessionState::Working;
                    self.tool_use_at = None;
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notifier_at(state: SessionState) -> JsonlNotifier {
        JsonlNotifier {
            jsonl_path: PathBuf::from("/nonexistent/none.jsonl"),
            last_known_size: 0,
            read_offset: 0,
            state,
            tool_use_at: None,
        }
    }

    fn line(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn test_end_turn_after_user_notifies_once() {
        let mut n = notifier_at(SessionState::Unknown);
        assert!(!n.apply_line(&line(r#"{"type":"user"}"#)));
        assert!(n.apply_line(&line(
            r#"{"type":"assistant","message":{"stop_reason":"end_turn"}}"#
        )));
        assert!(!n.apply_line(&line(
            r#"{"type":"assistant","message":{"stop_reason":"end_turn"}}"#
        )));
    }

    #[test]
    fn test_turn_duration_notifies_while_working() {
        let mut n = notifier_at(SessionState::Working);
        assert!(n.apply_line(&line(r#"{"type":"system","subtype":"turn_duration"}"#)));
        assert!(!n.apply_line(&line(r#"{"type":"system","subtype":"turn_duration"}"#)));
    }

    #[test]
    fn test_tool_use_starts_wait_timer_and_user_cancels_it() {
        let mut n = notifier_at(SessionState::UserSent);
        assert!(!n.apply_line(&line(
            r#"{"type":"assistant","message":{"stop_reason":"tool_use"}}"#
        )));
        assert!(n.tool_use_at.is_some());
        assert!(!n.tool_wait_expired());
        n.tool_use_at = Some(now_millis() - TOOL_WAIT_MS - 1);
        assert!(n.tool_wait_expired());
        assert!(n.tool_use_at.is_none());
    }

    #[test]
    fn test_check_reads_appended_lines_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, "").unwrap();
        let mut n = JsonlNotifier {
            jsonl_path: path.clone(),
            last_known_size: 0,
            read_offset: 0,
            state: SessionState::Unknown,
            tool_use_at: None,
        };
        assert!(!n.check());
        std::fs::write(
            &path,
            "{\"type\":\"user\"}\n{\"type\":\"assistant\",\"message\":{\"stop_reason\":\"end_turn\"}}\n",
        )
        .unwrap();
        assert!(n.check());
        assert!(!n.check());
    }

    #[test]
    fn test_notifier_waits_for_its_own_transcript() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("other.jsonl"),
            "{\"type\":\"user\"}\n{\"type\":\"assistant\",\"message\":{\"stop_reason\":\"end_turn\"}}\n",
        )
        .unwrap();
        let mut n = JsonlNotifier {
            jsonl_path: dir.path().join("mine.jsonl"),
            last_known_size: 0,
            read_offset: 0,
            state: SessionState::Unknown,
            tool_use_at: None,
        };
        assert!(!n.check(), "must not adopt another session's transcript");
        std::fs::write(
            dir.path().join("mine.jsonl"),
            "{\"type\":\"user\"}\n{\"type\":\"assistant\",\"message\":{\"stop_reason\":\"end_turn\"}}\n",
        )
        .unwrap();
        assert!(n.check());
    }
}
