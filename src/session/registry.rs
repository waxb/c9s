use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryStatus {
    Idle,
    Busy,
    Waiting,
    Compacting,
    Other(String),
}

impl RegistryStatus {
    fn parse(raw: &str) -> Self {
        match raw {
            "idle" => Self::Idle,
            "busy" => Self::Busy,
            "waiting" => Self::Waiting,
            "compacting" => Self::Compacting,
            other => Self::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub pid: u32,
    pub session_id: String,
    pub name: Option<String>,
    pub status: RegistryStatus,
    pub kind: String,
    pub cwd: Option<String>,
    pub messaging_socket_path: Option<String>,
    pub started_at: Option<u64>,
    pub waiting_for: Option<String>,
    pub name_is_custom: bool,
}

#[derive(Deserialize)]
struct RawEntry {
    pid: u32,
    #[serde(rename = "sessionId")]
    session_id: String,
    name: Option<String>,
    status: Option<String>,
    kind: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "messagingSocketPath")]
    messaging_socket_path: Option<String>,
    #[serde(rename = "startedAt")]
    started_at: Option<u64>,
    #[serde(rename = "waitingFor")]
    waiting_for: Option<String>,
    #[serde(rename = "nameSource")]
    name_source: Option<String>,
}

pub fn parse_entry(text: &str) -> Option<RegistryEntry> {
    let raw: RawEntry = serde_json::from_str(text).ok()?;
    Some(RegistryEntry {
        pid: raw.pid,
        session_id: raw.session_id,
        name: raw.name.filter(|n| !n.is_empty()),
        status: RegistryStatus::parse(raw.status.as_deref().unwrap_or("")),
        kind: raw.kind.unwrap_or_default(),
        cwd: raw.cwd,
        messaging_socket_path: raw.messaging_socket_path,
        started_at: raw.started_at,
        waiting_for: raw.waiting_for.filter(|w| !w.is_empty()),
        name_is_custom: matches!(raw.name_source.as_deref(), Some("user" | "collision")),
    })
}

pub fn process_alive(pid: u32) -> bool {
    let rc = unsafe { libc::kill(pid as i32, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub struct SessionRegistry {
    dir: PathBuf,
}

impl SessionRegistry {
    pub fn new() -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".claude")
            .join("sessions");
        Self { dir }
    }

    #[cfg(test)]
    pub fn with_dir(dir: &std::path::Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    pub fn live_interactive(&self) -> Vec<RegistryEntry> {
        self.entries(process_alive)
    }

    fn entries(&self, alive: impl Fn(u32) -> bool) -> Vec<RegistryEntry> {
        let Ok(dir) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        dir.flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .filter_map(|text| parse_entry(&text))
            .filter(|e| e.kind == "interactive" && alive(e.pid))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"pid":76606,"sessionId":"2e88-1","cwd":"/Users/me/proj","startedAt":1788857550258,"version":"2.1.260","kind":"interactive","name":"proj-c5","nameSource":"derived","status":"busy","messagingSocketPath":"/tmp/cc-socks/76606.sock"}"#;

    #[test]
    fn test_parse_entry() {
        let e = parse_entry(SAMPLE).unwrap();
        assert_eq!(e.pid, 76606);
        assert_eq!(e.session_id, "2e88-1");
        assert_eq!(e.name.as_deref(), Some("proj-c5"));
        assert_eq!(e.status, RegistryStatus::Busy);
        assert_eq!(e.kind, "interactive");
        assert_eq!(e.cwd.as_deref(), Some("/Users/me/proj"));
        assert_eq!(
            e.messaging_socket_path.as_deref(),
            Some("/tmp/cc-socks/76606.sock")
        );
        assert_eq!(e.started_at, Some(1788857550258));
        assert_eq!(e.waiting_for, None);
        assert!(!e.name_is_custom);
    }

    #[test]
    fn test_parse_entry_user_name_is_custom() {
        let e = parse_entry(
            r#"{"pid":1,"sessionId":"a","kind":"interactive","status":"idle","name":"brave-otter","nameSource":"user"}"#,
        )
        .unwrap();
        assert!(e.name_is_custom);
        let c = parse_entry(
            r#"{"pid":1,"sessionId":"a","kind":"interactive","name":"x-graceful-unicorn","nameSource":"collision"}"#,
        )
        .unwrap();
        assert!(c.name_is_custom);
    }

    #[test]
    fn test_parse_entry_waiting_for() {
        let e = parse_entry(
            r#"{"pid":1,"sessionId":"a","kind":"interactive","status":"waiting","waitingFor":"approve Bash"}"#,
        )
        .unwrap();
        assert_eq!(e.status, RegistryStatus::Waiting);
        assert_eq!(e.waiting_for.as_deref(), Some("approve Bash"));
    }

    #[test]
    fn test_parse_entry_unknown_status_and_missing_fields() {
        let e = parse_entry(r#"{"pid":1,"sessionId":"a","cwd":"/x","status":"weird"}"#).unwrap();
        assert_eq!(e.status, RegistryStatus::Other("weird".into()));
        assert_eq!(e.name, None);
        assert_eq!(e.kind, "");
        assert!(parse_entry("not json").is_none());
    }

    #[test]
    fn test_entries_filters_dead_and_non_interactive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("1.json"), SAMPLE).unwrap();
        std::fs::write(
            dir.path().join("2.json"),
            r#"{"pid":2,"sessionId":"dead","cwd":"/x","kind":"interactive","status":"idle"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("3.json"),
            r#"{"pid":3,"sessionId":"bg","cwd":"/x","kind":"background","status":"idle"}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("x.key"), "secret").unwrap();

        let registry = SessionRegistry::with_dir(dir.path());
        let entries = registry.entries(|pid| pid == 76606 || pid == 3);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "2e88-1");
    }

    #[test]
    fn test_process_alive_for_own_pid() {
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(u32::MAX - 1));
    }
}
