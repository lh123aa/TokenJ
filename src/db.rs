use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

/// # 性能设计
///
/// - 写操作通过 `tokio::task::spawn_blocking` 异步化，不阻塞 tokio 工作线程
/// - 读操作同步返回（调用方为 async，自动 yield）
/// - WAL 模式保证读写不互斥
/// - `busy_timeout=5000` 避免 SQLITE_BUSY 错误
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

        // 如果数据库文件是 0 字节（崩溃后留下的无效空文件），先删除再重建
        if db_path.exists() && std::fs::metadata(db_path)?.len() == 0 {
            std::fs::remove_file(db_path)?;
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

    /// 插入请求记录
    pub fn insert_request(&self, record: &RequestRecord) -> Result<()> {
        let conn = self.conn.lock().expect("Database mutex poisoned");
        let mut stmt = conn.prepare_cached(
            "INSERT INTO requests (id, session_id, provider, model, input_tokens, output_tokens,
             cached_tokens, cache_write_tokens, actual_cost_cents, saving_cents, saving_rate,
             cache_injected, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;
        stmt.execute(params![
            record.id, record.session_id, record.provider, record.model,
            record.input_tokens, record.output_tokens, record.cached_tokens,
            record.cache_write_tokens, record.actual_cost_cents, record.saving_cents,
            record.saving_rate, record.cache_injected as i32, record.duration_ms,
            record.created_at,
        ])?;
        Ok(())
    }

    pub fn create_session(&self) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().expect("Database mutex poisoned in create_session");
        let mut stmt = conn.prepare_cached(
            "INSERT INTO sessions (id, start_time) VALUES (?1, ?2)",
        )?;
        stmt.execute(params![id, now])?;
        Ok(id)
    }

    pub fn end_session(&self, session_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().expect("Mutex poisoned in end_session");
        let mut stmt = conn.prepare_cached(
            "UPDATE sessions SET end_time = ?1 WHERE id = ?2",
        )?;
        stmt.execute(params![now, session_id])?;
        Ok(())
    }

    pub fn get_stats_since(&self, since: &str) -> Result<SessionStats> {
        let conn = self.conn.lock().expect("Mutex poisoned in get_stats_since");
        let mut stmt = conn.prepare_cached(
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
        let mut stmt = conn.prepare_cached(
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

/// 通过 spawn_blocking 异步插入请求记录，不阻塞 tokio 工作线程
///
/// 使用示例（在 async 上下文中）:
/// ```ignore
/// tokio::task::spawn({
///     let db = db.clone();
///     let rec = record.clone();
///     async move { db.insert_request_blocking(rec).await }
/// });
/// ```
pub async fn insert_request_blocking(db: &std::sync::Arc<Database>, record: &RequestRecord) -> Result<()> {
    let db = std::sync::Arc::clone(db);
    let record = record.clone();
    tokio::task::spawn_blocking(move || {
        db.insert_request(&record)
    })
    .await
    .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {}", e))??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (Database, std::path::PathBuf) {
        let uid = Uuid::new_v4();
        let path = std::env::temp_dir().join(format!("TokenJ_db_test_{}.db", uid));
        let db = Database::new(&path).unwrap();
        (db, path)
    }

    fn sample_record() -> RequestRecord {
        RequestRecord {
            id: Uuid::new_v4().to_string(),
            session_id: "test-session".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-6".into(),
            input_tokens: 5000,
            output_tokens: 200,
            cached_tokens: 4500,
            cache_write_tokens: 0,
            actual_cost_cents: 0.30,
            saving_cents: 2.70,
            saving_rate: 90.0,
            cache_injected: true,
            duration_ms: 500,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn test_db_create_and_insert() {
        let (db, path) = temp_db();
        db.insert_request(&sample_record()).unwrap();
        let stats = db.get_stats_since("1970-01-01").unwrap();
        assert_eq!(stats.total_requests, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_db_recent_requests() {
        let (db, path) = temp_db();
        db.insert_request(&sample_record()).unwrap();
        let recent = db.get_recent_requests(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].provider, "anthropic");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_db_stats_multiple_records() {
        let (db, path) = temp_db();
        for _ in 0..5 {
            db.insert_request(&sample_record()).unwrap();
        }
        let stats = db.get_stats_since("1970-01-01").unwrap();
        assert_eq!(stats.total_requests, 5);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_db_session_lifecycle() {
        let (db, path) = temp_db();
        let session_id = db.create_session().unwrap();
        db.end_session(&session_id).unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_db_empty_stats() {
        let (db, path) = temp_db();
        let stats = db.get_stats_since("1970-01-01").unwrap();
        assert_eq!(stats.total_requests, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_db_multiple_providers() {
        let (db, path) = temp_db();
        for p in &["anthropic", "openai", "deepseek", "gemini"] {
            let mut rec = sample_record();
            rec.id = Uuid::new_v4().to_string();
            rec.provider = p.to_string();
            db.insert_request(&rec).unwrap();
        }
        let recent = db.get_recent_requests(10).unwrap();
        assert_eq!(recent.len(), 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_db_stats_filter_by_time() {
        let (db, path) = temp_db();
        db.insert_request(&sample_record()).unwrap();
        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let stats = db.get_stats_since(&future).unwrap();
        assert_eq!(stats.total_requests, 0);
        let _ = std::fs::remove_file(&path);
    }
}
