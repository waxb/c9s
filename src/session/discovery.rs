use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use super::{Session, SessionStatus};

pub struct SessionDiscovery {
    claude_dir: PathBuf,
    stats_cache: HashMap<PathBuf, (SystemTime, JsonlStats)>,
}

#[derive(Debug)]
struct ProcessInfo {
    pid: u32,
    cwd: PathBuf,
}

#[derive(Debug, Default, Clone)]
struct JsonlStats {
    session_id: Option<String>,
    cwd: Option<String>,
    git_branch: Option<String>,
    model: Option<String>,
    claude_version: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    message_count: u32,
    tool_call_count: u32,
    first_timestamp: Option<DateTime<Utc>>,
    last_timestamp: Option<DateTime<Utc>>,
    last_message_type: Option<String>,
    last_stop_reason: Option<String>,
    permission_mode: Option<String>,
    plan_slugs: Vec<String>,
    compaction_count: u32,
    hook_run_count: u32,
    hook_error_count: u32,
}

impl SessionDiscovery {
    pub fn new() -> Self {
        let claude_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".claude");
        Self {
            claude_dir,
            stats_cache: HashMap::new(),
        }
    }

    pub fn discover_all(&mut self) -> Result<Vec<Session>> {
        let live_processes = self.find_claude_processes()?;
        let mut sessions = Vec::new();

        let live_cwds: HashMap<String, u32> = live_processes
            .iter()
            .map(|p| (p.cwd.to_string_lossy().to_string(), p.pid))
            .collect();

        let projects_dir = self.claude_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(sessions);
        }

        let mut seen_sessions: HashMap<String, Session> = HashMap::new();

        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let dir_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if dir_name.starts_with('.') {
                    continue;
                }

                let fallback_cwd = decode_project_path(&dir_name);

                if let Ok(jsonl_files) = std::fs::read_dir(&path) {
                    for jf in jsonl_files.flatten() {
                        let jf_path = jf.path();
                        if jf_path.extension().is_none_or(|e| e != "jsonl") {
                            continue;
                        }

                        let session_id = jf_path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();

                        if session_id.contains('.') {
                            continue;
                        }

                        let stats = self.parse_jsonl_cached(&jf_path);

                        if stats.session_id.is_none() && stats.message_count == 0 {
                            continue;
                        }

                        let real_session_id = stats
                            .session_id
                            .clone()
                            .unwrap_or_else(|| session_id.clone());

                        let project_cwd = stats.cwd.clone().unwrap_or_else(|| fallback_cwd.clone());

                        let pid = live_cwds.get(&project_cwd).copied();

                        let status = match pid {
                            Some(_) => {
                                let hung_threshold = chrono::Duration::minutes(5);
                                let is_hung = match stats.last_timestamp {
                                    Some(ts) => {
                                        Utc::now().signed_duration_since(ts) > hung_threshold
                                    }
                                    None => false,
                                };
                                if is_hung {
                                    SessionStatus::Idle
                                } else {
                                    match stats.last_message_type.as_deref() {
                                        Some("user") => SessionStatus::Thinking,
                                        Some("assistant") => {
                                            match stats.last_stop_reason.as_deref() {
                                                Some("end_turn") => SessionStatus::Idle,
                                                Some("tool_use") => SessionStatus::Active,
                                                _ => SessionStatus::Active,
                                            }
                                        }
                                        _ => SessionStatus::Active,
                                    }
                                }
                            }
                            None => SessionStatus::Dead,
                        };

                        let project_name = extract_project_name(&project_cwd);
                        let now = Utc::now();

                        let session = Session {
                            id: real_session_id.clone(),
                            pid,
                            cwd: PathBuf::from(&project_cwd),
                            project_name,
                            git_branch: stats.git_branch,
                            model: stats.model,
                            status,
                            started_at: stats.first_timestamp.unwrap_or(now),
                            last_activity: stats.last_timestamp.unwrap_or(now),
                            input_tokens: stats.input_tokens,
                            output_tokens: stats.output_tokens,
                            cache_read_tokens: stats.cache_read_tokens,
                            cache_write_tokens: stats.cache_write_tokens,
                            message_count: stats.message_count,
                            tool_call_count: stats.tool_call_count,
                            claude_version: stats.claude_version,
                            permission_mode: stats.permission_mode,
                            plan_slugs: stats.plan_slugs,
                            compaction_count: stats.compaction_count,
                            hook_run_count: stats.hook_run_count,
                            hook_error_count: stats.hook_error_count,
                        };

                        let existing = seen_sessions.get(&project_cwd);
                        let should_insert = match existing {
                            None => true,
                            Some(ex) => session.last_activity > ex.last_activity,
                        };

                        if should_insert {
                            seen_sessions.insert(project_cwd.clone(), session);
                        }
                    }
                }
            }
        }

        sessions.extend(seen_sessions.into_values());
        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        Ok(sessions)
    }

    fn find_claude_processes(&self) -> Result<Vec<ProcessInfo>> {
        let pgrep_output = Command::new("pgrep").arg("-x").arg("claude").output();

        let pids: Vec<u32> = match pgrep_output {
            Ok(output) => String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.trim().parse().ok())
                .collect(),
            Err(_) => return Ok(Vec::new()),
        };

        if pids.is_empty() {
            return Ok(Vec::new());
        }

        self.resolve_process_cwds(&pids)
    }

    #[cfg(target_os = "macos")]
    fn resolve_process_cwds(&self, pids: &[u32]) -> Result<Vec<ProcessInfo>> {
        let pid_list = pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let output = Command::new("lsof")
            .args(["-p", &pid_list, "-a", "-d", "cwd", "-Fn"])
            .output()
            .context("failed to run lsof")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes = Vec::new();
        let mut current_pid: Option<u32> = None;

        for line in stdout.lines() {
            if let Some(pid_str) = line.strip_prefix('p') {
                current_pid = pid_str.parse().ok();
            } else if let Some(path) = line.strip_prefix('n') {
                if let Some(pid) = current_pid {
                    processes.push(ProcessInfo {
                        pid,
                        cwd: PathBuf::from(path),
                    });
                }
            }
        }

        Ok(processes)
    }

    #[cfg(target_os = "linux")]
    fn resolve_process_cwds(&self, pids: &[u32]) -> Result<Vec<ProcessInfo>> {
        let mut processes = Vec::new();
        for &pid in pids {
            if let Ok(link) = std::fs::read_link(format!("/proc/{}/cwd", pid)) {
                processes.push(ProcessInfo { pid, cwd: link });
            }
        }
        Ok(processes)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn resolve_process_cwds(&self, _pids: &[u32]) -> Result<Vec<ProcessInfo>> {
        Ok(Vec::new())
    }

    fn parse_jsonl_cached(&mut self, path: &Path) -> JsonlStats {
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();

        if let Some(mtime) = mtime {
            if let Some((cached_mtime, cached_stats)) = self.stats_cache.get(path) {
                if *cached_mtime == mtime {
                    return cached_stats.clone();
                }
            }
        }

        let stats = Self::parse_jsonl(path);

        if let Some(mtime) = mtime {
            self.stats_cache
                .insert(path.to_path_buf(), (mtime, stats.clone()));
        }

        stats
    }

    fn parse_jsonl(path: &Path) -> JsonlStats {
        let mut stats = JsonlStats::default();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return stats,
        };

        for line in content.lines() {
            let value: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if let Some(sid) = value.get("sessionId").and_then(|v| v.as_str()) {
                stats.session_id = Some(sid.to_string());
            }

            if stats.cwd.is_none() {
                if let Some(cwd) = value.get("cwd").and_then(|v| v.as_str()) {
                    stats.cwd = Some(cwd.to_string());
                }
            }

            if let Some(branch) = value.get("gitBranch").and_then(|v| v.as_str()) {
                stats.git_branch = Some(branch.to_string());
            }

            if let Some(version) = value.get("version").and_then(|v| v.as_str()) {
                stats.claude_version = Some(version.to_string());
            }

            if stats.permission_mode.is_none() {
                if let Some(pm) = value.get("permissionMode").and_then(|v| v.as_str()) {
                    stats.permission_mode = Some(pm.to_string());
                }
            }

            if let Some(slug) = value.get("slug").and_then(|v| v.as_str()) {
                if !stats.plan_slugs.contains(&slug.to_string()) {
                    stats.plan_slugs.push(slug.to_string());
                }
            }

            if value.get("isCompactSummary").and_then(|v| v.as_bool()) == Some(true) {
                stats.compaction_count += 1;
            }

            if let Some(hc) = value.get("hookCount").and_then(|v| v.as_u64()) {
                stats.hook_run_count += hc as u32;
            }

            if let Some(errs) = value.get("hookErrors").and_then(|v| v.as_array()) {
                stats.hook_error_count += errs.len() as u32;
            }

            if let Some(ts_str) = value.get("timestamp").and_then(|v| v.as_str()) {
                if let Ok(ts) = ts_str.parse::<DateTime<Utc>>() {
                    if stats.first_timestamp.is_none() {
                        stats.first_timestamp = Some(ts);
                    }
                    stats.last_timestamp = Some(ts);
                }
            }

            let msg_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match msg_type {
                "user" => {
                    stats.message_count += 1;
                    stats.last_message_type = Some("user".to_string());
                }
                "assistant" => {
                    stats.message_count += 1;
                    stats.last_message_type = Some("assistant".to_string());

                    if let Some(message) = value.get("message") {
                        if let Some(sr) = message.get("stop_reason").and_then(|v| v.as_str()) {
                            stats.last_stop_reason = Some(sr.to_string());
                        }
                        if let Some(model) = message.get("model").and_then(|v| v.as_str()) {
                            stats.model = Some(model.to_string());
                        }

                        if let Some(usage) = message.get("usage") {
                            stats.input_tokens += usage
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            stats.output_tokens += usage
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            stats.cache_read_tokens += usage
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            stats.cache_write_tokens += usage
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                        }
                    }
                }
                "progress" => {
                    if value.get("data").is_some() {
                        stats.tool_call_count += 1;
                    }
                }
                _ => {}
            }
        }

        stats
    }
}

fn decode_project_path(encoded: &str) -> String {
    encoded.replace('-', "/")
}

fn extract_project_name(cwd: &str) -> String {
    let parts: Vec<&str> = cwd.trim_end_matches('/').rsplit('/').collect();
    match parts.len() {
        0 => cwd.to_string(),
        1 => parts[0].to_string(),
        _ => format!("{}/{}", parts[1], parts[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_project_path() {
        assert_eq!(decode_project_path("-Users-foo-bar"), "/Users/foo/bar");
        assert_eq!(
            decode_project_path("-home-user-project"),
            "/home/user/project"
        );
    }

    #[test]
    fn test_parse_jsonl_extracts_session_id() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.jsonl");
        let lines = [
            r#"{"sessionId":"abc-123","cwd":"/tmp/proj","type":"user","timestamp":"2026-01-01T00:00:00Z"}"#,
            r#"{"type":"assistant","message":{"model":"claude-sonnet-4-20250514","stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":50}},"timestamp":"2026-01-01T00:01:00Z"}"#,
        ];
        std::fs::write(&file, lines.join("\n")).unwrap();

        let stats = SessionDiscovery::parse_jsonl(&file);
        assert_eq!(stats.session_id.as_deref(), Some("abc-123"));
        assert_eq!(stats.cwd.as_deref(), Some("/tmp/proj"));
        assert_eq!(stats.input_tokens, 100);
        assert_eq!(stats.output_tokens, 50);
        assert_eq!(stats.message_count, 2);
        assert!(stats.model.as_deref().unwrap().contains("sonnet"));
    }
}
