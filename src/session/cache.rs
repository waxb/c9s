use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::jsonl::{scan_transcript, JsonlStats};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachedTranscript {
    pub scanned_len: u64,
    pub file_len: u64,
    pub mtime_ms: i64,
    pub stats: JsonlStats,
}

#[derive(Default)]
pub struct TranscriptCache {
    entries: HashMap<PathBuf, CachedTranscript>,
    dirty: HashSet<PathBuf>,
}

fn file_signature(path: &Path) -> Option<(u64, i64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Some((meta.len(), mtime_ms))
}

impl TranscriptCache {
    pub fn seed(&mut self, entries: impl IntoIterator<Item = (PathBuf, CachedTranscript)>) {
        self.entries.extend(entries);
    }

    pub fn stats_for(&mut self, path: &Path) -> Option<JsonlStats> {
        let (file_len, mtime_ms) = file_signature(path)?;
        let cached = self.entries.get(path);

        if let Some(entry) = cached {
            if entry.file_len == file_len && entry.mtime_ms == mtime_ms {
                return Some(entry.stats.clone());
            }
        }

        let mut entry = match cached {
            Some(entry) if file_len >= entry.scanned_len => entry.clone(),
            _ => CachedTranscript {
                scanned_len: 0,
                file_len: 0,
                mtime_ms: 0,
                stats: JsonlStats::default(),
            },
        };
        entry.scanned_len = scan_transcript(path, entry.scanned_len, &mut entry.stats);
        entry.file_len = file_len;
        entry.mtime_ms = mtime_ms;

        let stats = entry.stats.clone();
        self.entries.insert(path.to_path_buf(), entry);
        self.dirty.insert(path.to_path_buf());
        Some(stats)
    }

    pub fn take_dirty(&mut self) -> Vec<(PathBuf, CachedTranscript)> {
        let dirty = std::mem::take(&mut self.dirty);
        dirty
            .into_iter()
            .filter_map(|p| self.entries.get(&p).map(|e| (p, e.clone())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const USER: &str =
        r#"{"sessionId":"s","type":"user","cwd":"/p","timestamp":"2026-01-01T00:00:00Z"}"#;
    const ASSISTANT: &str =
        r#"{"type":"assistant","message":{"usage":{"input_tokens":10,"output_tokens":2}}}"#;

    fn write_lines(path: &Path, lines: &[&str]) {
        let mut f = std::fs::File::create(path).unwrap();
        for l in lines {
            writeln!(f, "{}", l).unwrap();
        }
    }

    fn append_line(path: &Path, line: &str) {
        let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        writeln!(f, "{}", line).unwrap();
    }

    #[test]
    fn test_incremental_scan_on_growth_and_dirty_tracking() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        write_lines(&path, &[USER, ASSISTANT]);
        let mut cache = TranscriptCache::default();

        let first = cache.stats_for(&path).unwrap();
        assert_eq!(first.message_count, 2);
        assert_eq!(first.input_tokens, 10);
        assert_eq!(cache.take_dirty().len(), 1);
        assert!(cache.take_dirty().is_empty());

        let same = cache.stats_for(&path).unwrap();
        assert_eq!(same, first);
        assert!(cache.take_dirty().is_empty());

        std::thread::sleep(std::time::Duration::from_millis(20));
        append_line(&path, ASSISTANT);
        let grown = cache.stats_for(&path).unwrap();
        assert_eq!(grown.message_count, 3);
        assert_eq!(grown.input_tokens, 20);
        let dirty = cache.take_dirty();
        assert_eq!(dirty.len(), 1);
        assert_eq!(
            dirty[0].1.scanned_len,
            std::fs::metadata(&path).unwrap().len()
        );
    }

    #[test]
    fn test_rescans_from_start_when_file_shrinks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        write_lines(&path, &[USER, ASSISTANT, ASSISTANT]);
        let mut cache = TranscriptCache::default();
        assert_eq!(cache.stats_for(&path).unwrap().input_tokens, 20);

        std::thread::sleep(std::time::Duration::from_millis(20));
        write_lines(&path, &[USER]);
        let stats = cache.stats_for(&path).unwrap();
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.input_tokens, 0);
    }

    #[test]
    fn test_seeded_entry_skips_rescan_when_signature_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jsonl");
        write_lines(&path, &[USER, ASSISTANT]);
        let (len, mtime) = file_signature(&path).unwrap();
        let seeded = CachedTranscript {
            scanned_len: len,
            file_len: len,
            mtime_ms: mtime,
            stats: JsonlStats {
                message_count: 42,
                ..Default::default()
            },
        };
        let mut cache = TranscriptCache::default();
        cache.seed(vec![(path.clone(), seeded)]);
        assert_eq!(cache.stats_for(&path).unwrap().message_count, 42);
        assert!(cache.take_dirty().is_empty());

        std::thread::sleep(std::time::Duration::from_millis(20));
        append_line(&path, ASSISTANT);
        assert_eq!(cache.stats_for(&path).unwrap().message_count, 43);
    }

    #[test]
    fn test_missing_file_yields_none() {
        let mut cache = TranscriptCache::default();
        assert!(cache.stats_for(Path::new("/nonexistent/x.jsonl")).is_none());
    }
}
