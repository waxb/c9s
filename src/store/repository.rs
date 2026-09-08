use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::session::Session;

const SCHEMA_VERSION: i64 = 2;

const V2_COLUMNS: &[(&str, &str)] = &[
    ("title", "TEXT"),
    ("session_name", "TEXT"),
    ("context_tokens", "INTEGER DEFAULT 0"),
    ("compaction_count", "INTEGER DEFAULT 0"),
    ("api_error_count", "INTEGER DEFAULT 0"),
    ("cache_write_5m_tokens", "INTEGER DEFAULT 0"),
    ("cache_write_1h_tokens", "INTEGER DEFAULT 0"),
    ("hidden", "INTEGER DEFAULT 0"),
];

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open() -> Result<Self> {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".c9s");
        std::fs::create_dir_all(&data_dir)?;
        Self::open_at(&data_dir.join("data.db"))
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                cwd TEXT NOT NULL,
                project_name TEXT NOT NULL,
                git_branch TEXT,
                model TEXT,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                total_input_tokens INTEGER DEFAULT 0,
                total_output_tokens INTEGER DEFAULT 0,
                total_cache_read_tokens INTEGER DEFAULT 0,
                total_cache_write_tokens INTEGER DEFAULT 0,
                estimated_cost_usd REAL DEFAULT 0.0,
                message_count INTEGER DEFAULT 0,
                tool_call_count INTEGER DEFAULT 0,
                claude_version TEXT
            );

            CREATE TABLE IF NOT EXISTS daily_stats (
                date TEXT PRIMARY KEY,
                total_sessions INTEGER DEFAULT 0,
                total_messages INTEGER DEFAULT 0,
                total_tool_calls INTEGER DEFAULT 0,
                total_input_tokens INTEGER DEFAULT 0,
                total_output_tokens INTEGER DEFAULT 0,
                total_cost_usd REAL DEFAULT 0.0
            );

            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_name);
            CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);",
        )?;

        let existing = self.session_columns()?;
        for (name, decl) in V2_COLUMNS {
            if !existing.contains(*name) {
                self.conn.execute(
                    &format!("ALTER TABLE sessions ADD COLUMN {} {}", name, decl),
                    [],
                )?;
            }
        }

        self.conn.execute("DELETE FROM schema_version", [])?;
        self.conn.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            rusqlite::params![SCHEMA_VERSION],
        )?;
        Ok(())
    }

    fn session_columns(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(sessions)")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<HashSet<String>, _>>()?;
        Ok(names)
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> Result<i64> {
        let version: i64 =
            self.conn
                .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                    row.get(0)
                })?;
        Ok(version)
    }

    pub fn upsert_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, cwd, project_name, git_branch, model, started_at,
                total_input_tokens, total_output_tokens, total_cache_read_tokens,
                total_cache_write_tokens, estimated_cost_usd, message_count,
                tool_call_count, claude_version, title, session_name, context_tokens,
                compaction_count, api_error_count, cache_write_5m_tokens, cache_write_1h_tokens)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                ?18, ?19, ?20, ?21)
            ON CONFLICT(id) DO UPDATE SET
                cwd = excluded.cwd,
                project_name = excluded.project_name,
                git_branch = excluded.git_branch,
                model = excluded.model,
                total_input_tokens = excluded.total_input_tokens,
                total_output_tokens = excluded.total_output_tokens,
                total_cache_read_tokens = excluded.total_cache_read_tokens,
                total_cache_write_tokens = excluded.total_cache_write_tokens,
                estimated_cost_usd = excluded.estimated_cost_usd,
                message_count = excluded.message_count,
                tool_call_count = excluded.tool_call_count,
                claude_version = excluded.claude_version,
                title = excluded.title,
                session_name = excluded.session_name,
                context_tokens = excluded.context_tokens,
                compaction_count = excluded.compaction_count,
                api_error_count = excluded.api_error_count,
                cache_write_5m_tokens = excluded.cache_write_5m_tokens,
                cache_write_1h_tokens = excluded.cache_write_1h_tokens",
            rusqlite::params![
                session.id,
                session.cwd.to_string_lossy(),
                session.project_name,
                session.git_branch,
                session.model,
                session.started_at.to_rfc3339(),
                session.input_tokens,
                session.output_tokens,
                session.cache_read_tokens,
                session.cache_write_tokens(),
                session.estimated_cost_usd(),
                session.message_count,
                session.tool_call_count,
                session.claude_version,
                session.title,
                session.session_name,
                session.context_tokens,
                session.compaction_count,
                session.api_error_count,
                session.cache_write_5m_tokens,
                session.cache_write_1h_tokens,
            ],
        )?;
        Ok(())
    }

    pub fn set_hidden(&self, session_id: &str, hidden: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, cwd, project_name, started_at, hidden)
            VALUES (?1, '', '', datetime('now'), ?2)
            ON CONFLICT(id) DO UPDATE SET hidden = excluded.hidden",
            rusqlite::params![session_id, hidden as i64],
        )?;
        Ok(())
    }

    pub fn hidden_ids(&self) -> Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM sessions WHERE hidden = 1")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<HashSet<String>, _>>()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::test_support::make_session;

    fn temp_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_at(&dir.path().join("data.db")).unwrap();
        (dir, store)
    }

    #[test]
    fn test_migrate_is_idempotent_and_sets_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.db");
        let first = Store::open_at(&path).unwrap();
        assert_eq!(first.schema_version().unwrap(), SCHEMA_VERSION);
        drop(first);
        let second = Store::open_at(&path).unwrap();
        assert_eq!(second.schema_version().unwrap(), SCHEMA_VERSION);
        let columns = second.session_columns().unwrap();
        for (name, _) in V2_COLUMNS {
            assert!(columns.contains(*name), "missing column {}", name);
        }
    }

    #[test]
    fn test_migrate_upgrades_v1_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE sessions (
                    id TEXT PRIMARY KEY, cwd TEXT NOT NULL, project_name TEXT NOT NULL,
                    git_branch TEXT, model TEXT, started_at TEXT NOT NULL, ended_at TEXT,
                    total_input_tokens INTEGER DEFAULT 0, total_output_tokens INTEGER DEFAULT 0,
                    total_cache_read_tokens INTEGER DEFAULT 0,
                    total_cache_write_tokens INTEGER DEFAULT 0,
                    estimated_cost_usd REAL DEFAULT 0.0, message_count INTEGER DEFAULT 0,
                    tool_call_count INTEGER DEFAULT 0, claude_version TEXT);
                INSERT INTO sessions (id, cwd, project_name, started_at)
                VALUES ('old', '/p', 'p', '2026-01-01T00:00:00Z');",
            )
            .unwrap();
        }
        let store = Store::open_at(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(store.hidden_ids().unwrap().is_empty());
        store.set_hidden("old", true).unwrap();
        assert!(store.hidden_ids().unwrap().contains("old"));
    }

    #[test]
    fn test_upsert_keeps_hidden_flag() {
        let (_dir, store) = temp_store();
        let mut s = make_session("s1", "claude-opus-5");
        s.title = Some("Fix bug".into());
        s.context_tokens = 1234;
        store.upsert_session(&s).unwrap();
        store.set_hidden("s1", true).unwrap();
        store.upsert_session(&s).unwrap();
        assert!(store.hidden_ids().unwrap().contains("s1"));
        store.set_hidden("s1", false).unwrap();
        assert!(store.hidden_ids().unwrap().is_empty());

        let title: String = store
            .conn
            .query_row("SELECT title FROM sessions WHERE id = 's1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(title, "Fix bug");
    }

    #[test]
    fn test_set_hidden_for_unknown_session_creates_row() {
        let (_dir, store) = temp_store();
        store.set_hidden("ghost", true).unwrap();
        assert!(store.hidden_ids().unwrap().contains("ghost"));
    }
}
