use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct Database {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct RequestRecord {
    pub id: String,
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub actual_cost_cents: f64,
    pub saving_cents: f64,
    pub saving_rate: f64,
    pub cache_injected: bool,
    pub duration_ms: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub total_requests: u64,
    pub total_cost_cents: f64,
    pub total_saving_cents: f64,
    pub total_cached_tokens: u64,
    pub total_cache_write_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_hit_rate: f64,
    pub avg_saving_rate: f64,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS requests (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cached_tokens INTEGER NOT NULL DEFAULT 0,
                cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                actual_cost_cents REAL NOT NULL DEFAULT 0,
                saving_cents REAL NOT NULL DEFAULT 0,
                saving_rate REAL NOT NULL DEFAULT 0,
                cache_injected INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                start_time TEXT NOT NULL,
                end_time TEXT,
                total_requests INTEGER NOT NULL DEFAULT 0,
                total_cost_cents REAL NOT NULL DEFAULT 0,
                total_saving_cents REAL NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_requests_created ON requests(created_at);
            CREATE INDEX IF NOT EXISTS idx_requests_session ON requests(session_id);
            CREATE INDEX IF NOT EXISTS idx_requests_provider ON requests(provider);",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_request(&self, record: &RequestRecord) -> Result<()> {
        let conn = self.conn.lock().expect("Database mutex poisoned");
        conn.execute(
            "INSERT INTO requests (id, session_id, provider, model, input_tokens, output_tokens,
             cached_tokens, cache_write_tokens, actual_cost_cents, saving_cents, saving_rate,
             cache_injected, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                record.id,
                record.session_id,
                record.provider,
                record.model,
                record.input_tokens,
                record.output_tokens,
                record.cached_tokens,
                record.cache_write_tokens,
                record.actual_cost_cents,
                record.saving_cents,
                record.saving_rate,
                record.cache_injected as i32,
                record.duration_ms,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn create_session(&self) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().expect("Database mutex poisoned in create_session");
        conn.execute(
            "INSERT INTO sessions (id, start_time) VALUES (?1, ?2)",
            params![id, now],
        )?;
        Ok(id)
    }

    pub fn end_session(&self, session_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().expect("Mutex poisoned in end_session");
        conn.execute(
            "UPDATE sessions SET end_time = ?1 WHERE id = ?2",
            params![now, session_id],
        )?;
        Ok(())
    }

    pub fn get_stats_since(&self, since: &str) -> Result<SessionStats> {
        let conn = self.conn.lock().expect("Mutex poisoned in get_stats_since");
        let mut stmt = conn.prepare(
            "SELECT
                COUNT(*) as total_requests,
                COALESCE(SUM(actual_cost_cents), 0) as total_cost,
                COALESCE(SUM(saving_cents), 0) as total_saving,
                COALESCE(SUM(cached_tokens), 0) as total_cached,
                COALESCE(SUM(cache_write_tokens), 0) as total_writes,
                COALESCE(SUM(input_tokens), 0) as total_input,
                COALESCE(SUM(output_tokens), 0) as total_output,
                CASE WHEN SUM(cached_tokens + cache_write_tokens) > 0
                     THEN CAST(SUM(cached_tokens) AS REAL) / SUM(cached_tokens + cache_write_tokens) * 100
                     ELSE 0 END as hit_rate,
                CASE WHEN SUM(actual_cost_cents) > 0
                     THEN CAST(SUM(saving_cents) AS REAL) / (SUM(actual_cost_cents) + SUM(saving_cents)) * 100
                     ELSE 0 END as avg_saving
             FROM requests WHERE created_at >= ?1",
        )?;

        let stats = stmt.query_row(params![since], |row| {
            Ok(SessionStats {
                total_requests: row.get(0)?,
                total_cost_cents: row.get(1)?,
                total_saving_cents: row.get(2)?,
                total_cached_tokens: row.get(3)?,
                total_cache_write_tokens: row.get(4)?,
                total_input_tokens: row.get(5)?,
                total_output_tokens: row.get(6)?,
                cache_hit_rate: row.get::<_, f64>(7).unwrap_or(0.0),
                avg_saving_rate: row.get::<_, f64>(8).unwrap_or(0.0),
            })
        })?;

        Ok(stats)
    }

    pub fn get_recent_requests(&self, limit: u64) -> Result<Vec<RequestRecord>> {
        let conn = self.conn.lock().expect("Mutex poisoned in get_recent_requests");
        let mut stmt = conn.prepare(
            "SELECT id, session_id, provider, model, input_tokens, output_tokens,
             cached_tokens, cache_write_tokens, actual_cost_cents, saving_cents, saving_rate,
             cache_injected, duration_ms, created_at
             FROM requests ORDER BY created_at DESC LIMIT ?1",
        )?;

        let records = stmt
            .query_map(params![limit], |row| {
                Ok(RequestRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    provider: row.get(2)?,
                    model: row.get(3)?,
                    input_tokens: row.get(4)?,
                    output_tokens: row.get(5)?,
                    cached_tokens: row.get(6)?,
                    cache_write_tokens: row.get(7)?,
                    actual_cost_cents: row.get(8)?,
                    saving_cents: row.get(9)?,
                    saving_rate: row.get(10)?,
                    cache_injected: row.get::<_, i32>(11)? != 0,
                    duration_ms: row.get(12)?,
                    created_at: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }
}
