use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::Path;

const TAIL_SIZE: u64 = 512 * 1024;
const HEAD_LIMIT: u64 = 64 * 1024;
const LARGE_FILE_THRESHOLD: u64 = 2 * 1024 * 1024;

#[derive(Debug, Default, Clone)]
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

pub fn parse_jsonl(path: &Path) -> JsonlStats {
    let mut stats = JsonlStats::default();
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    if file_size > LARGE_FILE_THRESHOLD {
        parse_jsonl_fast(path, file_size, &mut stats);
    } else if let Ok(content) = std::fs::read_to_string(path) {
        parse_jsonl_lines(&content, &mut stats, true);
    }

    stats
}

fn parse_jsonl_fast(path: &Path, file_size: u64, stats: &mut JsonlStats) {
    use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let mut reader = BufReader::new(file);

    let mut head = String::new();
    let mut head_bytes = 0u64;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                head_bytes += n as u64;
                head.push_str(&line);
                if head_bytes >= HEAD_LIMIT {
                    break;
                }
            }
        }
    }
    parse_jsonl_lines(&head, stats, false);

    let tail_start = file_size.saturating_sub(TAIL_SIZE);
    if tail_start <= head_bytes {
        return;
    }
    if reader.seek(SeekFrom::Start(tail_start)).is_err() {
        return;
    }
    let mut tail = String::new();
    if reader.read_to_string(&mut tail).is_err() {
        return;
    }
    if let Some(first_nl) = tail.find('\n') {
        parse_jsonl_lines(&tail[first_nl + 1..], stats, false);
    }
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|v| v.as_str())
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

pub fn parse_jsonl_lines(content: &str, stats: &mut JsonlStats, count_messages: bool) {
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        apply_common_fields(&value, stats);

        match str_field(&value, "type").unwrap_or("") {
            "user" => {
                if count_messages {
                    stats.message_count += 1;
                }
                stats.last_message_type = Some("user".to_string());
            }
            "assistant" => {
                if count_messages {
                    stats.message_count += 1;
                }
                stats.last_message_type = Some("assistant".to_string());
                if let Some(message) = value.get("message") {
                    let is_sidechain = value
                        .get("isSidechain")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    apply_assistant_message(message, is_sidechain, stats);
                }
            }
            "system" => match str_field(&value, "subtype") {
                Some("compact_boundary") => stats.compaction_count += 1,
                Some("api_error") => stats.api_error_count += 1,
                _ => {}
            },
            "ai-title" => {
                stats.ai_title = str_field(&value, "aiTitle").map(str::to_string);
            }
            "custom-title" => {
                stats.custom_title = str_field(&value, "customTitle").map(str::to_string);
            }
            "last-prompt" => {
                stats.last_prompt = str_field(&value, "lastPrompt").map(str::to_string);
            }
            _ => {}
        }
    }
}

fn apply_common_fields(value: &Value, stats: &mut JsonlStats) {
    if let Some(sid) = str_field(value, "sessionId") {
        stats.session_id = Some(sid.to_string());
    }
    if stats.cwd.is_none() {
        if let Some(cwd) = str_field(value, "cwd") {
            stats.cwd = Some(cwd.to_string());
        }
    }
    if let Some(branch) = str_field(value, "gitBranch") {
        stats.git_branch = Some(branch.to_string());
    }
    if let Some(version) = str_field(value, "version") {
        stats.claude_version = Some(version.to_string());
    }
    if stats.permission_mode.is_none() {
        if let Some(pm) = str_field(value, "permissionMode") {
            stats.permission_mode = Some(pm.to_string());
        }
    }
    if let Some(slug) = str_field(value, "slug") {
        if !stats.plan_slugs.iter().any(|s| s == slug) {
            stats.plan_slugs.push(slug.to_string());
        }
    }
    if let Some(hc) = value.get("hookCount").and_then(|v| v.as_u64()) {
        stats.hook_run_count += hc as u32;
    }
    if let Some(errs) = value.get("hookErrors").and_then(|v| v.as_array()) {
        stats.hook_error_count += errs.len() as u32;
    }
    if let Some(ts) = str_field(value, "timestamp").and_then(|s| s.parse::<DateTime<Utc>>().ok()) {
        if stats.first_timestamp.is_none() {
            stats.first_timestamp = Some(ts);
        }
        stats.last_timestamp = Some(ts);
    }
}

fn apply_assistant_message(message: &Value, is_sidechain: bool, stats: &mut JsonlStats) {
    if let Some(sr) = str_field(message, "stop_reason") {
        stats.last_stop_reason = Some(sr.to_string());
    }
    if let Some(model) = str_field(message, "model") {
        stats.model = Some(model.to_string());
    }
    if let Some(blocks) = message.get("content").and_then(|v| v.as_array()) {
        let tool_uses = blocks
            .iter()
            .filter(|b| str_field(b, "type") == Some("tool_use"))
            .count();
        stats.tool_call_count += tool_uses as u32;
    }
    let Some(usage) = message.get("usage") else {
        return;
    };

    let input = u64_field(usage, "input_tokens");
    let output = u64_field(usage, "output_tokens");
    let cache_read = u64_field(usage, "cache_read_input_tokens");
    let cache_creation = u64_field(usage, "cache_creation_input_tokens");
    let (write_5m, write_1h) = split_cache_creation(usage, cache_creation);

    stats.input_tokens += input;
    stats.output_tokens += output;
    stats.cache_read_tokens += cache_read;
    stats.cache_write_5m_tokens += write_5m;
    stats.cache_write_1h_tokens += write_1h;

    if !is_sidechain {
        stats.context_tokens = input + cache_read + cache_creation;
    }
}

fn split_cache_creation(usage: &Value, total: u64) -> (u64, u64) {
    let breakdown = usage.get("cache_creation");
    let w5 = breakdown
        .map(|b| u64_field(b, "ephemeral_5m_input_tokens"))
        .unwrap_or(0);
    let w1h = breakdown
        .map(|b| u64_field(b, "ephemeral_1h_input_tokens"))
        .unwrap_or(0);
    if w5 + w1h == 0 {
        (total, 0)
    } else {
        (w5, w1h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats_for(lines: &[&str]) -> JsonlStats {
        let mut stats = JsonlStats::default();
        parse_jsonl_lines(&lines.join("\n"), &mut stats, true);
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
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"},{"type":"tool_use","id":"1","name":"Bash","input":{}},{"type":"tool_use","id":"2","name":"Read","input":{}}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"3","name":"Edit","input":{}}]}}"#,
        ]);
        assert_eq!(stats.tool_call_count, 3);
    }

    #[test]
    fn test_titles_and_last_prompt() {
        let stats = stats_for(&[
            r#"{"type":"ai-title","aiTitle":"First title","sessionId":"s"}"#,
            r#"{"type":"ai-title","aiTitle":"Second title","sessionId":"s"}"#,
            r#"{"type":"last-prompt","lastPrompt":"do the thing","sessionId":"s"}"#,
        ]);
        assert_eq!(stats.title().as_deref(), Some("Second title"));
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
    fn test_parse_jsonl_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.jsonl");
        std::fs::write(
            &file,
            r#"{"sessionId":"x","type":"user","cwd":"/p","timestamp":"2026-01-01T00:00:00Z","gitBranch":"main","version":"2.1.0"}"#,
        )
        .unwrap();
        let stats = parse_jsonl(&file);
        assert_eq!(stats.session_id.as_deref(), Some("x"));
        assert_eq!(stats.git_branch.as_deref(), Some("main"));
        assert_eq!(stats.claude_version.as_deref(), Some("2.1.0"));
        assert_eq!(stats.message_count, 1);
    }
}
