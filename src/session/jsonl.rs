use chrono::{DateTime, Utc};
use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonlStats {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub model: Option<String>,
    pub claude_version: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub message_count: u32,
    pub tool_call_count: u32,
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub last_message_type: Option<String>,
    pub last_stop_reason: Option<String>,
    pub permission_mode: Option<String>,
    pub plan_slugs: Vec<String>,
    pub compaction_count: u32,
    pub hook_run_count: u32,
    pub hook_error_count: u32,
    pub api_error_count: u32,
    pub ai_title: Option<String>,
    pub custom_title: Option<String>,
    pub last_prompt: Option<String>,
    pub context_tokens: u64,
}

impl JsonlStats {
    pub fn title(&self) -> Option<String> {
        self.custom_title.clone().or_else(|| self.ai_title.clone())
    }
}

#[derive(Deserialize)]
struct RawLine<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
    #[serde(rename = "sessionId", borrow)]
    session_id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    cwd: Option<Cow<'a, str>>,
    #[serde(rename = "gitBranch", borrow)]
    git_branch: Option<Cow<'a, str>>,
    #[serde(borrow)]
    version: Option<Cow<'a, str>>,
    #[serde(rename = "permissionMode", borrow)]
    permission_mode: Option<Cow<'a, str>>,
    #[serde(borrow)]
    slug: Option<Cow<'a, str>>,
    #[serde(rename = "hookCount")]
    hook_count: Option<u64>,
    #[serde(rename = "hookErrors")]
    hook_errors: Option<Vec<IgnoredAny>>,
    #[serde(borrow)]
    timestamp: Option<Cow<'a, str>>,
    #[serde(borrow)]
    subtype: Option<Cow<'a, str>>,
    #[serde(rename = "isSidechain")]
    is_sidechain: Option<bool>,
    #[serde(rename = "aiTitle", borrow)]
    ai_title: Option<Cow<'a, str>>,
    #[serde(rename = "customTitle", borrow)]
    custom_title: Option<Cow<'a, str>>,
    #[serde(rename = "lastPrompt", borrow)]
    last_prompt: Option<Cow<'a, str>>,
    #[serde(borrow)]
    message: Option<RawMessage<'a>>,
}

#[derive(Deserialize)]
struct RawMessage<'a> {
    #[serde(borrow)]
    model: Option<Cow<'a, str>>,
    #[serde(borrow)]
    stop_reason: Option<Cow<'a, str>>,
    usage: Option<RawUsage>,
    #[serde(default, deserialize_with = "tool_use_count")]
    content: u32,
}

#[derive(Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    cache_creation: Option<RawCacheCreation>,
}

#[derive(Deserialize, Default)]
struct RawCacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

#[derive(Deserialize)]
struct RawBlock<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
}

fn tool_use_count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    struct ContentVisitor;

    impl<'de> Visitor<'de> for ContentVisitor {
        type Value = u32;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a content string or an array of content blocks")
        }

        fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<u32, E> {
            Ok(0)
        }

        fn visit_borrowed_str<E: serde::de::Error>(self, _: &'de str) -> Result<u32, E> {
            Ok(0)
        }

        fn visit_string<E: serde::de::Error>(self, _: String) -> Result<u32, E> {
            Ok(0)
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<u32, E> {
            Ok(0)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<u32, E> {
            Ok(0)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<u32, A::Error> {
            let mut count = 0;
            while let Some(block) = seq.next_element::<RawBlock<'de>>()? {
                if block.kind.as_deref() == Some("tool_use") {
                    count += 1;
                }
            }
            Ok(count)
        }
    }

    deserializer.deserialize_any(ContentVisitor)
}

fn owned(value: &Option<Cow<'_, str>>) -> Option<String> {
    value.as_deref().map(str::to_string)
}

fn apply_line(line: &[u8], stats: &mut JsonlStats) {
    let Ok(raw) = serde_json::from_slice::<RawLine>(line) else {
        return;
    };
    apply_common_fields(&raw, stats);

    match raw.kind.as_deref().unwrap_or("") {
        "user" => {
            stats.message_count += 1;
            stats.last_message_type = Some("user".to_string());
        }
        "assistant" => {
            stats.message_count += 1;
            stats.last_message_type = Some("assistant".to_string());
            if let Some(message) = &raw.message {
                apply_assistant_message(message, raw.is_sidechain.unwrap_or(false), stats);
            }
        }
        "system" => match raw.subtype.as_deref() {
            Some("compact_boundary") => stats.compaction_count += 1,
            Some("api_error") => stats.api_error_count += 1,
            _ => {}
        },
        "ai-title" => stats.ai_title = owned(&raw.ai_title),
        "custom-title" => stats.custom_title = owned(&raw.custom_title),
        "last-prompt" => stats.last_prompt = owned(&raw.last_prompt),
        _ => {}
    }
}

fn apply_common_fields(raw: &RawLine, stats: &mut JsonlStats) {
    if let Some(sid) = owned(&raw.session_id) {
        stats.session_id = Some(sid);
    }
    if stats.cwd.is_none() {
        stats.cwd = owned(&raw.cwd);
    }
    if let Some(branch) = owned(&raw.git_branch) {
        stats.git_branch = Some(branch);
    }
    if let Some(version) = owned(&raw.version) {
        stats.claude_version = Some(version);
    }
    if stats.permission_mode.is_none() {
        stats.permission_mode = owned(&raw.permission_mode);
    }
    if let Some(slug) = raw.slug.as_deref() {
        if !stats.plan_slugs.iter().any(|s| s == slug) {
            stats.plan_slugs.push(slug.to_string());
        }
    }
    if let Some(hc) = raw.hook_count {
        stats.hook_run_count += hc as u32;
    }
    if let Some(errs) = &raw.hook_errors {
        stats.hook_error_count += errs.len() as u32;
    }
    if let Some(ts) = raw
        .timestamp
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
    {
        if stats.first_timestamp.is_none() {
            stats.first_timestamp = Some(ts);
        }
        stats.last_timestamp = Some(ts);
    }
}

fn apply_assistant_message(message: &RawMessage, is_sidechain: bool, stats: &mut JsonlStats) {
    if let Some(sr) = owned(&message.stop_reason) {
        stats.last_stop_reason = Some(sr);
    }
    if let Some(model) = owned(&message.model) {
        stats.model = Some(model);
    }
    stats.tool_call_count += message.content;

    let Some(usage) = &message.usage else {
        return;
    };
    let (write_5m, write_1h) = split_cache_creation(usage);
    stats.input_tokens += usage.input_tokens;
    stats.output_tokens += usage.output_tokens;
    stats.cache_read_tokens += usage.cache_read_input_tokens;
    stats.cache_write_5m_tokens += write_5m;
    stats.cache_write_1h_tokens += write_1h;

    if !is_sidechain {
        stats.context_tokens =
            usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens;
    }
}

fn split_cache_creation(usage: &RawUsage) -> (u64, u64) {
    match &usage.cache_creation {
        Some(b) if b.ephemeral_5m_input_tokens + b.ephemeral_1h_input_tokens > 0 => {
            (b.ephemeral_5m_input_tokens, b.ephemeral_1h_input_tokens)
        }
        _ => (usage.cache_creation_input_tokens, 0),
    }
}

#[cfg(test)]
pub fn parse_jsonl_lines(content: &str, stats: &mut JsonlStats) {
    for line in content.lines() {
        if !line.trim().is_empty() {
            apply_line(line.as_bytes(), stats);
        }
    }
}

pub fn scan_transcript(path: &Path, from: u64, stats: &mut JsonlStats) -> u64 {
    let Ok(mut file) = std::fs::File::open(path) else {
        return from;
    };
    if file.seek(SeekFrom::Start(from)).is_err() {
        return from;
    }
    let mut reader = BufReader::with_capacity(256 * 1024, file);
    let mut buf = Vec::new();
    let mut offset = from;
    loop {
        buf.clear();
        let Ok(n) = reader.read_until(b'\n', &mut buf) else {
            break;
        };
        if n == 0 || buf.last() != Some(&b'\n') {
            break;
        }
        apply_line(&buf[..n - 1], stats);
        offset += n as u64;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn stats_for(lines: &[&str]) -> JsonlStats {
        let mut stats = JsonlStats::default();
        parse_jsonl_lines(&lines.join("\n"), &mut stats);
        stats
    }

    #[test]
    fn test_extracts_session_id_and_usage() {
        let stats = stats_for(&[
            r#"{"sessionId":"abc-123","cwd":"/tmp/proj","type":"user","timestamp":"2026-01-01T00:00:00Z"}"#,
            r#"{"type":"assistant","message":{"model":"claude-sonnet-4-20250514","stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":50}},"timestamp":"2026-01-01T00:01:00Z"}"#,
        ]);
        assert_eq!(stats.session_id.as_deref(), Some("abc-123"));
        assert_eq!(stats.cwd.as_deref(), Some("/tmp/proj"));
        assert_eq!(stats.input_tokens, 100);
        assert_eq!(stats.output_tokens, 50);
        assert_eq!(stats.message_count, 2);
        assert!(stats.model.as_deref().unwrap().contains("sonnet"));
        assert_eq!(stats.context_tokens, 100);
        assert_eq!(stats.last_stop_reason.as_deref(), Some("end_turn"));
        assert!(stats.first_timestamp.is_some());
    }

    #[test]
    fn test_counts_compactions_and_api_errors() {
        let stats = stats_for(&[
            r#"{"type":"system","subtype":"compact_boundary","compactMetadata":{"preTokens":300489}}"#,
            r#"{"type":"system","subtype":"turn_duration","durationMs":41}"#,
            r#"{"type":"system","subtype":"api_error"}"#,
            r#"{"type":"system","subtype":"compact_boundary"}"#,
        ]);
        assert_eq!(stats.compaction_count, 2);
        assert_eq!(stats.api_error_count, 1);
    }

    #[test]
    fn test_counts_tool_use_blocks_not_progress_lines() {
        let stats = stats_for(&[
            r#"{"type":"progress","data":{"type":"hook_progress"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"},{"type":"tool_use","id":"1","name":"Bash","input":{"cmd":"x"}},{"type":"tool_use","id":"2","name":"Read","input":{}}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"3","name":"Edit","input":{}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":"plain string content"}}"#,
        ]);
        assert_eq!(stats.tool_call_count, 3);
        assert_eq!(stats.message_count, 3);
    }

    #[test]
    fn test_titles_and_last_prompt_with_escapes() {
        let stats = stats_for(&[
            r#"{"type":"ai-title","aiTitle":"First title","sessionId":"s"}"#,
            r#"{"type":"ai-title","aiTitle":"Second \"quoted\" title","sessionId":"s"}"#,
            r#"{"type":"last-prompt","lastPrompt":"do the thing","sessionId":"s"}"#,
        ]);
        assert_eq!(stats.title().as_deref(), Some("Second \"quoted\" title"));
        assert_eq!(stats.last_prompt.as_deref(), Some("do the thing"));

        let custom = stats_for(&[
            r#"{"type":"ai-title","aiTitle":"Generated","sessionId":"s"}"#,
            r#"{"type":"custom-title","customTitle":"Mine","sessionId":"s"}"#,
        ]);
        assert_eq!(custom.title().as_deref(), Some("Mine"));
    }

    #[test]
    fn test_context_tokens_track_last_main_thread_assistant() {
        let stats = stats_for(&[
            r#"{"type":"assistant","message":{"usage":{"input_tokens":10,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}"#,
            r#"{"type":"assistant","message":{"usage":{"input_tokens":5,"cache_read_input_tokens":2000,"cache_creation_input_tokens":300}}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"usage":{"input_tokens":1,"cache_read_input_tokens":99999,"cache_creation_input_tokens":0}}}"#,
        ]);
        assert_eq!(stats.context_tokens, 2305);
        assert_eq!(stats.cache_read_tokens, 1000 + 2000 + 99999);
    }

    #[test]
    fn test_cache_creation_breakdown() {
        let stats = stats_for(&[
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1,"cache_creation_input_tokens":500,"cache_creation":{"ephemeral_5m_input_tokens":100,"ephemeral_1h_input_tokens":400}}}}"#,
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1,"cache_creation_input_tokens":50}}}"#,
        ]);
        assert_eq!(stats.cache_write_5m_tokens, 150);
        assert_eq!(stats.cache_write_1h_tokens, 400);
    }

    #[test]
    fn test_hooks_slugs_and_malformed_lines() {
        let stats = stats_for(&[
            r#"{"type":"system","subtype":"stop_hook_summary","hookCount":2,"hookErrors":[{"x":1}],"slug":"vivid-frog"}"#,
            "not json at all",
            r#"{"type":"system","subtype":"turn_duration","slug":"vivid-frog","hookCount":1,"hookErrors":[]}"#,
            r#"{"type":"user","message":"unexpected shape"}"#,
        ]);
        assert_eq!(stats.hook_run_count, 3);
        assert_eq!(stats.hook_error_count, 1);
        assert_eq!(stats.plan_slugs, vec!["vivid-frog".to_string()]);
        assert_eq!(stats.message_count, 0);
    }

    #[test]
    fn test_scan_transcript_is_incremental_and_skips_partial_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("t.jsonl");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(
            f,
            r#"{{"sessionId":"x","type":"user","cwd":"/p","timestamp":"2026-01-01T00:00:00Z","gitBranch":"main","version":"2.1.0"}}"#
        )
        .unwrap();
        write!(
            f,
            r#"{{"type":"assistant","message":{{"usage":{{"input_tokens":7,"output_tokens":3}}}}}}"#
        )
        .unwrap();

        let mut stats = JsonlStats::default();
        let offset = scan_transcript(&file, 0, &mut stats);
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.input_tokens, 0);
        assert!(offset > 0);
        assert!(offset < std::fs::metadata(&file).unwrap().len());

        writeln!(f).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"usage":{{"input_tokens":5,"output_tokens":1}}}}}}"#
        )
        .unwrap();
        let end = scan_transcript(&file, offset, &mut stats);
        assert_eq!(end, std::fs::metadata(&file).unwrap().len());
        assert_eq!(stats.message_count, 3);
        assert_eq!(stats.input_tokens, 12);
        assert_eq!(stats.output_tokens, 4);
        assert_eq!(stats.git_branch.as_deref(), Some("main"));
        assert_eq!(stats.claude_version.as_deref(), Some("2.1.0"));

        let again = scan_transcript(&file, end, &mut stats);
        assert_eq!(again, end);
        assert_eq!(stats.message_count, 3);
    }

    #[test]
    fn test_stats_round_trip_json() {
        let stats = stats_for(&[
            r#"{"sessionId":"x","type":"user","cwd":"/p","timestamp":"2026-01-01T00:00:00Z"}"#,
        ]);
        let text = serde_json::to_string(&stats).unwrap();
        let back: JsonlStats = serde_json::from_str(&text).unwrap();
        assert_eq!(back, stats);
    }
}
