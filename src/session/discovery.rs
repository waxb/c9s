#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use super::jsonl::{parse_jsonl, JsonlStats};
use super::{Session, SessionStatus};

const HUNG_THRESHOLD_MINUTES: i64 = 5;
const CACHE_LARGE_FILE: u64 = 2 * 1024 * 1024;
const CACHE_GROWTH_TOLERANCE: u64 = 64 * 1024;

pub struct SessionDiscovery {
    claude_dir: PathBuf,
    stats_cache: HashMap<PathBuf, (SystemTime, u64, JsonlStats)>,
}

#[derive(Debug)]
struct ProcessInfo {
    pid: u32,
    cwd: PathBuf,
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
        let live_cwds: HashMap<String, u32> = self
            .find_claude_processes()?
            .into_iter()
            .map(|p| (p.cwd.to_string_lossy().to_string(), p.pid))
            .collect();

        let projects_dir = self.claude_dir.join("projects");
        let mut seen: HashMap<String, Session> = HashMap::new();

        for jf_path in list_session_jsonl_files(&projects_dir) {
            let fallback_cwd = jf_path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| decode_project_path(&n.to_string_lossy()))
                .unwrap_or_default();
            let stats = self.parse_jsonl_cached(&jf_path);
            if stats.session_id.is_none() && stats.message_count == 0 {
                continue;
            }
            let Some(session) = build_session(&jf_path, stats, &fallback_cwd, &live_cwds) else {
                continue;
            };
            let key = session.cwd.to_string_lossy().to_string();
            let replace = seen
                .get(&key)
                .is_none_or(|existing| prefer_over(&session, existing));
            if replace {
                seen.insert(key, session);
            }
        }

        let mut sessions: Vec<Session> = seen.into_values().collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_activity));
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

        Ok(parse_lsof_output(&String::from_utf8_lossy(&output.stdout)))
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
        let meta = std::fs::metadata(path).ok();
        let mtime = meta.as_ref().and_then(|m| m.modified().ok());
        let file_size = meta.as_ref().map(|m| m.len()).unwrap_or(0);

        if let Some(mtime) = mtime {
            if let Some((cached_mtime, cached_size, cached_stats)) = self.stats_cache.get(path) {
                if *cached_mtime == mtime {
                    return cached_stats.clone();
                }
                if file_size > CACHE_LARGE_FILE
                    && file_size.saturating_sub(*cached_size) < CACHE_GROWTH_TOLERANCE
                {
                    return cached_stats.clone();
                }
            }
        }

        let stats = parse_jsonl(path);

        if let Some(mtime) = mtime {
            self.stats_cache
                .insert(path.to_path_buf(), (mtime, file_size, stats.clone()));
        }

        stats
    }
}

#[cfg(target_os = "macos")]
fn parse_lsof_output(stdout: &str) -> Vec<ProcessInfo> {
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
    processes
}

fn list_session_jsonl_files(projects_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(projects_dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for project in entries.flatten() {
        let path = project.path();
        let name = project.file_name().to_string_lossy().to_string();
        if !path.is_dir() || name.starts_with('.') {
            continue;
        }
        let Ok(jsonl_files) = std::fs::read_dir(&path) else {
            continue;
        };
        files.extend(
            jsonl_files
                .flatten()
                .map(|e| e.path())
                .filter(|p| is_session_jsonl(p)),
        );
    }
    files
}

fn is_session_jsonl(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
        && path
            .file_stem()
            .is_some_and(|stem| !stem.to_string_lossy().contains('.'))
}

fn build_session(
    jf_path: &Path,
    stats: JsonlStats,
    fallback_cwd: &str,
    live_cwds: &HashMap<String, u32>,
) -> Option<Session> {
    let file_session_id = jf_path.file_stem()?.to_string_lossy().to_string();
    let id = stats.session_id.clone().unwrap_or(file_session_id);
    let project_cwd = stats
        .cwd
        .clone()
        .unwrap_or_else(|| fallback_cwd.to_string());
    let pid = live_cwds.get(&project_cwd).copied();
    let status = match pid {
        Some(_) => infer_live_status(&stats, Utc::now()),
        None => SessionStatus::Dead,
    };
    let now = Utc::now();

    Some(Session {
        id,
        pid,
        cwd: PathBuf::from(&project_cwd),
        project_name: extract_project_name(&project_cwd),
        git_branch: stats.git_branch.clone(),
        model: stats.model.clone(),
        status,
        started_at: stats.first_timestamp.unwrap_or(now),
        last_activity: stats.last_timestamp.unwrap_or(now),
        input_tokens: stats.input_tokens,
        output_tokens: stats.output_tokens,
        cache_read_tokens: stats.cache_read_tokens,
        cache_write_5m_tokens: stats.cache_write_5m_tokens,
        cache_write_1h_tokens: stats.cache_write_1h_tokens,
        context_tokens: stats.context_tokens,
        message_count: stats.message_count,
        tool_call_count: stats.tool_call_count,
        claude_version: stats.claude_version.clone(),
        permission_mode: stats.permission_mode.clone(),
        plan_slugs: stats.plan_slugs.clone(),
        compaction_count: stats.compaction_count,
        hook_run_count: stats.hook_run_count,
        hook_error_count: stats.hook_error_count,
        api_error_count: stats.api_error_count,
        title: stats.title(),
        last_prompt: stats.last_prompt.clone(),
    })
}

fn infer_live_status(stats: &JsonlStats, now: DateTime<Utc>) -> SessionStatus {
    let hung = stats.last_timestamp.is_some_and(|ts| {
        now.signed_duration_since(ts) > chrono::Duration::minutes(HUNG_THRESHOLD_MINUTES)
    });
    if hung {
        return SessionStatus::Idle;
    }
    match (
        stats.last_message_type.as_deref(),
        stats.last_stop_reason.as_deref(),
    ) {
        (Some("user"), _) => SessionStatus::Thinking,
        (Some("assistant"), Some("end_turn")) => SessionStatus::Idle,
        _ => SessionStatus::Active,
    }
}

fn prefer_over(candidate: &Session, existing: &Session) -> bool {
    let candidate_live = candidate.status != SessionStatus::Dead;
    let existing_live = existing.status != SessionStatus::Dead;
    match (candidate_live, existing_live) {
        (true, false) => true,
        (false, true) => false,
        _ => candidate.last_activity > existing.last_activity,
    }
}

#[derive(Debug, Clone)]
pub struct SessionFile {
    pub session_id: String,
    pub size_bytes: u64,
    pub last_modified: Option<DateTime<Utc>>,
    pub message_count: u32,
    pub is_current: bool,
}

pub fn list_session_files(cwd: &Path, current_session_id: &str) -> Vec<SessionFile> {
    let claude_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".claude");

    let encoded = cwd.to_string_lossy().replace('/', "-");
    let project_dir = claude_dir.join("projects").join(&encoded);

    let Ok(entries) = std::fs::read_dir(&project_dir) else {
        return Vec::new();
    };

    let mut files: Vec<SessionFile> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_session_jsonl(p))
        .filter_map(|path| session_file_entry(&path, current_session_id))
        .collect();

    files.sort_by_key(|f| std::cmp::Reverse(f.last_modified));
    files
}

fn session_file_entry(path: &Path, current_session_id: &str) -> Option<SessionFile> {
    let session_id = path.file_stem()?.to_string_lossy().to_string();
    let meta = std::fs::metadata(path).ok()?;
    let size_bytes = meta.len();
    if size_bytes == 0 {
        return None;
    }
    let is_current = session_id == current_session_id;
    let message_count = count_messages(path);
    if message_count == 0 && !is_current {
        return None;
    }
    Some(SessionFile {
        session_id,
        size_bytes,
        last_modified: meta.modified().ok().map(DateTime::<Utc>::from),
        message_count,
        is_current,
    })
}

fn count_messages(path: &Path) -> u32 {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    content
        .lines()
        .filter(|l| l.contains("\"type\":\"user\"") || l.contains("\"type\":\"assistant\""))
        .count() as u32
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
    fn test_extract_project_name() {
        assert_eq!(
            extract_project_name("/Users/me/Documents/c9s"),
            "Documents/c9s"
        );
        assert_eq!(extract_project_name("/c9s"), "/c9s");
    }

    #[test]
    fn test_is_session_jsonl() {
        assert!(is_session_jsonl(Path::new("/x/abc.jsonl")));
        assert!(!is_session_jsonl(Path::new("/x/abc.tmp.jsonl")));
        assert!(!is_session_jsonl(Path::new("/x/abc.json")));
    }

    #[test]
    fn test_infer_live_status() {
        let now = Utc::now();
        let fresh = JsonlStats {
            last_timestamp: Some(now),
            last_message_type: Some("user".to_string()),
            ..Default::default()
        };
        assert_eq!(infer_live_status(&fresh, now), SessionStatus::Thinking);

        let ended = JsonlStats {
            last_timestamp: Some(now),
            last_message_type: Some("assistant".to_string()),
            last_stop_reason: Some("end_turn".to_string()),
            ..Default::default()
        };
        assert_eq!(infer_live_status(&ended, now), SessionStatus::Idle);

        let stale = JsonlStats {
            last_timestamp: Some(now - chrono::Duration::minutes(10)),
            last_message_type: Some("user".to_string()),
            ..Default::default()
        };
        assert_eq!(infer_live_status(&stale, now), SessionStatus::Idle);
    }

    #[test]
    fn test_prefer_live_session_over_newer_dead_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        std::fs::write(&path, "").unwrap();
        let mut live_cwds = HashMap::new();
        live_cwds.insert("/p".to_string(), 42u32);

        let old_live = build_session(
            &path,
            JsonlStats {
                session_id: Some("a".into()),
                cwd: Some("/p".into()),
                last_timestamp: Some(Utc::now() - chrono::Duration::hours(2)),
                ..Default::default()
            },
            "",
            &live_cwds,
        )
        .unwrap();
        let new_dead = build_session(
            &path,
            JsonlStats {
                session_id: Some("b".into()),
                cwd: Some("/p".into()),
                last_timestamp: Some(Utc::now()),
                ..Default::default()
            },
            "",
            &HashMap::new(),
        )
        .unwrap();
        assert!(prefer_over(&old_live, &new_dead));
        assert!(!prefer_over(&new_dead, &old_live));
    }

    #[test]
    fn test_discover_all_from_temp_claude_dir() {
        let home = tempfile::tempdir().unwrap();
        let project_dir = home.path().join("projects").join("-tmp-proj");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(
            project_dir.join("s1.jsonl"),
            r#"{"sessionId":"s1","cwd":"/tmp/proj","type":"user","timestamp":"2026-01-01T00:00:00Z"}
{"type":"assistant","message":{"model":"claude-opus-4-6","usage":{"input_tokens":10,"output_tokens":5}},"timestamp":"2026-01-01T00:01:00Z"}
{"type":"ai-title","aiTitle":"Fix bug","sessionId":"s1"}"#,
        )
        .unwrap();
        let mut discovery = SessionDiscovery {
            claude_dir: home.path().to_path_buf(),
            stats_cache: HashMap::new(),
        };
        let sessions = discovery.discover_all().unwrap();
        let s = sessions.iter().find(|s| s.id == "s1").expect("s1 found");
        assert_eq!(s.cwd, PathBuf::from("/tmp/proj"));
        assert_eq!(s.title.as_deref(), Some("Fix bug"));
        assert_eq!(s.message_count, 2);
    }
}
