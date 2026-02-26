use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

use crate::session::Session;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open() -> Result<Self> {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".c9s");
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("data.db");
        let conn = Connection::open(&db_path)?;
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

            CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_name);
            CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);",
        )?;
        Ok(())
    }

    pub fn upsert_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, cwd, project_name, git_branch, model, started_at,
                total_input_tokens, total_output_tokens, total_cache_read_tokens,
                total_cache_write_tokens, estimated_cost_usd, message_count,
                tool_call_count, claude_version)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(id) DO UPDATE SET
                git_branch = excluded.git_branch,
                model = excluded.model,
                total_input_tokens = excluded.total_input_tokens,
                total_output_tokens = excluded.total_output_tokens,
                total_cache_read_tokens = excluded.total_cache_read_tokens,
                total_cache_write_tokens = excluded.total_cache_write_tokens,
                estimated_cost_usd = excluded.estimated_cost_usd,
                message_count = excluded.message_count,
                tool_call_count = excluded.tool_call_count,
                claude_version = excluded.claude_version",
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
                session.cache_write_tokens,
                session.estimated_cost_usd(),
                session.message_count,
                session.tool_call_count,
                session.claude_version,
            ],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn mark_session_ended(&self, session_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET ended_at = datetime('now') WHERE id = ?1 AND ended_at IS NULL",
            rusqlite::params![session_id],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_total_cost(&self) -> Result<f64> {
        let cost: f64 = self
            .conn
            .query_row("SELECT COALESCE(SUM(estimated_cost_usd), 0.0) FROM sessions", [], |row| {
                row.get(0)
            })?;
        Ok(cost)
    }

    #[allow(dead_code)]
    pub fn get_today_cost(&self) -> Result<f64> {
        let cost: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) FROM sessions WHERE date(started_at) = date('now')",
            [],
            |row| row.get(0),
        )?;
        Ok(cost)
    }

    #[allow(dead_code)]
    pub fn get_session_count(&self) -> Result<u64> {
        let count: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
        Ok(count)
    }
}
