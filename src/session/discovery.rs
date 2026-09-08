#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::cache::{CachedTranscript, TranscriptCache};
use super::jsonl::JsonlStats;
use super::registry::{RegistryEntry, RegistryStatus, SessionRegistry};
use super::{Session, SessionStatus};

const HUNG_THRESHOLD_MINUTES: i64 = 5;

pub struct SessionDiscovery {
    claude_dir: PathBuf,
    registry: SessionRegistry,
    cache: TranscriptCache,
}

struct LiveIndex {
    by_session: HashMap<String, RegistryEntry>,
    by_cwd: HashMap<String, u32>,
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
            registry: SessionRegistry::new(),
            cache: TranscriptCache::default(),
        }
    }

    #[cfg(test)]
    pub fn with_dirs(claude_dir: &Path, sessions_dir: &Path) -> Self {
        Self {
            claude_dir: claude_dir.to_path_buf(),
            registry: SessionRegistry::with_dir(sessions_dir),
            cache: TranscriptCache::default(),
        }
    }

    fn live_index(&self) -> Result<LiveIndex> {
        let by_session: HashMap<String, RegistryEntry> = self
            .registry
            .live_interactive()
            .into_iter()
            .map(|e| (e.session_id.clone(), e))
            .collect();
        let known_pids: HashSet<u32> = by_session.values().map(|e| e.pid).collect();
        let by_cwd = self
            .find_claude_processes(&known_pids)?
            .into_iter()
            .map(|p| (p.cwd.to_string_lossy().to_string(), p.pid))
            .collect();
        Ok(LiveIndex { by_session, by_cwd })
    }

    pub fn discover_all(&mut self) -> Result<Vec<Session>> {
        let live = self.live_index()?;

        let projects_dir = self.claude_dir.join("projects");
        let mut sessions: Vec<Session> = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        for jf_path in list_session_jsonl_files(&projects_dir) {
            let fallback_cwd = jf_path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| decode_project_path(&n.to_string_lossy()))
                .unwrap_or_default();
            let Some(stats) = self.cache.stats_for(&jf_path) else {
                continue;
            };
            if stats.session_id.is_none() && stats.message_count == 0 {
                continue;
            }
            let Some(session) = build_session(&jf_path, stats, &fallback_cwd, &live) else {
                continue;
            };
            if seen_ids.insert(session.id.clone()) {
                sessions.push(session);
            }
        }

        for entry in live.by_session.values() {
            if !seen_ids.contains(&entry.session_id) {
                sessions.push(placeholder_session(entry));
            }
        }

        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_activity));
        Ok(sessions)
    }

    fn find_claude_processes(&self, known_pids: &HashSet<u32>) -> Result<Vec<ProcessInfo>> {
        let pgrep_output = Command::new("pgrep").arg("-x").arg("claude").output();

        let pids: Vec<u32> = match pgrep_output {
            Ok(output) => String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.trim().parse().ok())
                .filter(|pid| !known_pids.contains(pid))
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

    pub fn seed_cache(&mut self, entries: Vec<(PathBuf, CachedTranscript)>) {
        self.cache.seed(entries);
    }

    pub fn take_dirty_cache(&mut self) -> Vec<(PathBuf, CachedTranscript)> {
        self.cache.take_dirty()
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

struct Liveness {
    pid: Option<u32>,
    pid_verified: bool,
    status: SessionStatus,
    session_name: Option<String>,
    messaging_socket: Option<String>,
}

fn resolve_liveness(id: &str, project_cwd: &str, stats: &JsonlStats, live: &LiveIndex) -> Liveness {
    if let Some(entry) = live.by_session.get(id) {
        return Liveness {
            pid: Some(entry.pid),
            pid_verified: true,
            status: status_from_registry(&entry.status),
            session_name: entry.name.clone(),
            messaging_socket: entry.messaging_socket_path.clone(),
        };
    }
    let pid = live.by_cwd.get(project_cwd).copied();
    Liveness {
        pid,
        pid_verified: false,
        status: match pid {
            Some(_) => infer_live_status(stats, Utc::now()),
            None => SessionStatus::Dead,
        },
        session_name: None,
        messaging_socket: None,
    }
}

fn status_from_registry(status: &RegistryStatus) -> SessionStatus {
    match status {
        RegistryStatus::Idle => SessionStatus::Idle,
        RegistryStatus::Waiting => SessionStatus::Waiting,
        RegistryStatus::Busy | RegistryStatus::Compacting | RegistryStatus::Other(_) => {
            SessionStatus::Active
        }
    }
}

fn build_session(
    jf_path: &Path,
    stats: JsonlStats,
    fallback_cwd: &str,
    live: &LiveIndex,
) -> Option<Session> {
    let file_session_id = jf_path.file_stem()?.to_string_lossy().to_string();
    let id = stats.session_id.clone().unwrap_or(file_session_id);
    let project_cwd = stats
        .cwd
        .clone()
        .unwrap_or_else(|| fallback_cwd.to_string());
    let liveness = resolve_liveness(&id, &project_cwd, &stats, live);
    let now = Utc::now();

    Some(Session {
        id,
        pid: liveness.pid,
        pid_verified: liveness.pid_verified,
        session_name: liveness.session_name,
        messaging_socket: liveness.messaging_socket,
        status: liveness.status,
        cwd: PathBuf::from(&project_cwd),
        project_name: extract_project_name(&project_cwd),
        git_branch: stats.git_branch.clone(),
        model: stats.model.clone(),
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

fn placeholder_session(entry: &RegistryEntry) -> Session {
    let cwd = entry.cwd.clone().unwrap_or_default();
    let started = entry
        .started_at
        .and_then(|ms| DateTime::<Utc>::from_timestamp_millis(ms as i64))
        .unwrap_or_else(Utc::now);
    Session {
        id: entry.session_id.clone(),
        pid: Some(entry.pid),
        pid_verified: true,
        session_name: entry.name.clone(),
        messaging_socket: entry.messaging_socket_path.clone(),
        status: status_from_registry(&entry.status),
        cwd: PathBuf::from(&cwd),
        project_name: extract_project_name(&cwd),
        git_branch: None,
        model: None,
        started_at: started,
        last_activity: started,
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

    fn live_index(by_cwd: &[(&str, u32)], entries: Vec<RegistryEntry>) -> LiveIndex {
        LiveIndex {
            by_session: entries
                .into_iter()
                .map(|e| (e.session_id.clone(), e))
                .collect(),
            by_cwd: by_cwd.iter().map(|(c, p)| (c.to_string(), *p)).collect(),
        }
    }

    fn stats(id: &str, cwd: &str, last: DateTime<Utc>) -> JsonlStats {
        JsonlStats {
            session_id: Some(id.into()),
            cwd: Some(cwd.into()),
            last_timestamp: Some(last),
            ..Default::default()
        }
    }

    #[test]
    fn test_registry_entry_gives_verified_pid_and_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        std::fs::write(&path, "").unwrap();
        let entry = RegistryEntry {
            pid: 4242,
            session_id: "a".into(),
            name: Some("proj-1a".into()),
            status: RegistryStatus::Waiting,
            kind: "interactive".into(),
            cwd: Some("/p".into()),
            messaging_socket_path: Some("/tmp/cc-socks/4242.sock".into()),
            started_at: None,
        };
        let live = live_index(&[("/p", 99)], vec![entry]);
        let s = build_session(&path, stats("a", "/p", Utc::now()), "", &live).unwrap();
        assert_eq!(s.pid, Some(4242));
        assert!(s.pid_verified);
        assert_eq!(s.status, SessionStatus::Waiting);
        assert_eq!(s.session_name.as_deref(), Some("proj-1a"));
        assert_eq!(
            s.messaging_socket.as_deref(),
            Some("/tmp/cc-socks/4242.sock")
        );

        let other = build_session(&path, stats("b", "/p", Utc::now()), "", &live).unwrap();
        assert_eq!(other.pid, Some(99));
        assert!(!other.pid_verified);
        assert_eq!(other.takeover_pid(), None);
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
{"type":"ai-title","aiTitle":"Fix bug","sessionId":"s1"}
"#,
        )
        .unwrap();
        let sessions_dir = home.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        std::fs::write(
            sessions_dir.join("1.json"),
            format!(
                r#"{{"pid":{},"sessionId":"s1","cwd":"/tmp/proj","kind":"interactive","status":"busy","name":"proj-aa"}}"#,
                std::process::id()
            ),
        )
        .unwrap();
        std::fs::write(
            project_dir.join("s2.jsonl"),
            "{\"sessionId\":\"s2\",\"cwd\":\"/tmp/proj\",\"type\":\"user\",\"timestamp\":\"2025-12-01T00:00:00Z\"}\n",
        )
        .unwrap();
        std::fs::write(
            sessions_dir.join("2.json"),
            format!(
                r#"{{"pid":{},"sessionId":"fresh","cwd":"/tmp/other","kind":"interactive","status":"busy","name":"other-bb","startedAt":1700000000000}}"#,
                std::process::id()
            ),
        )
        .unwrap();
        let mut discovery = SessionDiscovery::with_dirs(home.path(), &sessions_dir);
        let sessions = discovery.discover_all().unwrap();
        assert_eq!(
            sessions.len(),
            3,
            "both transcripts plus the registry-only session"
        );
        assert_eq!(sessions[0].id, "s1");
        let fresh = sessions
            .iter()
            .find(|s| s.id == "fresh")
            .expect("placeholder");
        assert_eq!(fresh.cwd, PathBuf::from("/tmp/other"));
        assert_eq!(fresh.session_name.as_deref(), Some("other-bb"));
        assert_eq!(fresh.status, SessionStatus::Active);
        assert!(fresh.pid_verified);
        assert_eq!(fresh.started_at.timestamp_millis(), 1700000000000);
        let s = sessions.iter().find(|s| s.id == "s1").expect("s1 found");
        assert_eq!(s.cwd, PathBuf::from("/tmp/proj"));
        assert_eq!(s.title.as_deref(), Some("Fix bug"));
        assert_eq!(s.message_count, 2);
        assert_eq!(s.status, SessionStatus::Active);
        assert_eq!(s.pid, Some(std::process::id()));
        assert!(s.pid_verified);
        assert_eq!(s.session_name.as_deref(), Some("proj-aa"));
    }
}
