use serde_json::Value;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn debug_log(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/c9s-bell-debug.log")
    {
        let ts = now_millis();
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

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
    jsonl_path: Option<PathBuf>,
    project_dir: PathBuf,
    last_size: u64,
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
        let (path, size) = if jsonl_path.exists() {
            let size = std::fs::metadata(&jsonl_path).map(|m| m.len()).unwrap_or(0);
            debug_log(&format!(
                "new: found jsonl at {:?} size={}",
                jsonl_path, size
            ));
            (Some(jsonl_path), size)
        } else {
            debug_log(&format!("new: no jsonl at {:?}, will discover", jsonl_path));
            (None, 0)
        };

        Self {
            jsonl_path: path,
            project_dir,
            last_size: size,
            state: SessionState::Unknown,
            tool_use_at: None,
        }
    }

    pub fn check(&mut self) -> bool {
        if self.jsonl_path.is_none() {
            self.try_discover_path();
        }

        let path = match &self.jsonl_path {
            Some(p) => p.clone(),
            None => return false,
        };

        let current_size = match std::fs::metadata(&path) {
            Ok(m) => m.len(),
            Err(_) => return false,
        };

        if current_size > self.last_size {
            debug_log(&format!(
                "new data: {} -> {} (+{})",
                self.last_size,
                current_size,
                current_size - self.last_size
            ));
        }

        if current_size == self.last_size {
            if let Some(tool_at) = self.tool_use_at {
                let elapsed = now_millis().saturating_sub(tool_at);
                if elapsed >= TOOL_WAIT_MS {
                    debug_log(&format!("BELL: tool_wait timer fired after {}ms", elapsed));
                    self.tool_use_at = None;
                    return true;
                }
            }
            return false;
        }

        if current_size < self.last_size {
            self.last_size = current_size;
            return false;
        }

        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        let mut reader = BufReader::new(file);
        if reader.seek(SeekFrom::Start(self.last_size)).is_err() {
            return false;
        }

        let mut should_notify = false;
        let mut line = String::new();

        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }

            let value: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => {
                    line.clear();
                    continue;
                }
            };

            if value.get("isCompactSummary").and_then(|v| v.as_bool()) == Some(true) {
                should_notify = true;
                self.tool_use_at = None;
                line.clear();
                continue;
            }

            let msg_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match msg_type {
                "user" => {
                    if self.tool_use_at.is_some() {
                        debug_log("user msg cancelled tool_wait timer");
                    }
                    self.state = SessionState::UserSent;
                    self.tool_use_at = None;
                }
                "assistant" => {
                    let stop_reason = value
                        .get("message")
                        .and_then(|m| m.get("stop_reason"))
                        .and_then(|s| s.as_str());

                    match stop_reason {
                        Some("end_turn") => {
                            if matches!(
                                self.state,
                                SessionState::UserSent
                                    | SessionState::Working
                                    | SessionState::ToolWait
                            ) {
                                debug_log(&format!(
                                    "BELL: end_turn from state {:?}",
                                    self.state as u8
                                ));
                                should_notify = true;
                            }
                            self.state = SessionState::Idle;
                            self.tool_use_at = None;
                        }
                        Some("tool_use") => {
                            debug_log("state -> ToolWait, starting 5s timer");
                            self.state = SessionState::ToolWait;
                            self.tool_use_at = Some(now_millis());
                        }
                        _ => {
                            if self.state != SessionState::ToolWait {
                                self.state = SessionState::Working;
                                self.tool_use_at = None;
                            }
                        }
                    }
                }
                "progress" | "result" => {
                    if self.state != SessionState::ToolWait {
                        self.state = SessionState::Working;
                    }
                }
                _ => {}
            }

            line.clear();
        }

        self.last_size = current_size;
        should_notify
    }

    pub fn debug_log_ext(&self, msg: &str) {
        debug_log(msg);
    }

    fn try_discover_path(&mut self) {
        if !self.project_dir.exists() {
            return;
        }

        let entries = match std::fs::read_dir(&self.project_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        let mut newest: Option<(PathBuf, SystemTime)> = None;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            if let Ok(meta) = p.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if newest.as_ref().is_none_or(|(_, t)| mtime > *t) {
                        newest = Some((p, mtime));
                    }
                }
            }
        }

        if let Some((path, _)) = newest {
            self.last_size = path.metadata().map(|m| m.len()).unwrap_or(0);
            self.jsonl_path = Some(path);
        }
    }
}
