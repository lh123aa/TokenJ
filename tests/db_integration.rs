use std::path::PathBuf;
use uuid::Uuid;

/// 辅助函数：创建临时数据库路径
fn temp_db_path() -> PathBuf {
    let uid = Uuid::new_v4();
    std::env::temp_dir().join(format!("TokenJ_int_test_{}.db", uid))
}

/// 辅助函数：创建示例请求记录
fn sample_record(_db: &TokenJ::db::Database) -> TokenJ::db::RequestRecord {
    TokenJ::db::RequestRecord {
        id: Uuid::new_v4().to_string(),
        session_id: "int-test".into(),
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
fn test_db_full_lifecycle() {
    let path = temp_db_path();
    let db = TokenJ::db::Database::new(&path).unwrap();

    // 创建 session
    let session_id = db.create_session().unwrap();
    assert!(!session_id.is_empty());

    // 插入请求
    let mut rec = sample_record(&db);
    rec.session_id = session_id.clone();
    db.insert_request(&rec).unwrap();

    // 验证统计
    let stats = db.get_stats_since("1970-01-01").unwrap();
    assert_eq!(stats.total_requests, 1);
    assert!(stats.total_cost_cents > 0.0);
    assert!(stats.total_saving_cents > 0.0);
    assert!(stats.cache_hit_rate > 0.0);

    // 查询最近请求
    let recent = db.get_recent_requests(10).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].provider, "anthropic");

    // 结束 session
    db.end_session(&session_id).unwrap();

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_db_multi_provider_stats() {
    let path = temp_db_path();
    let db = TokenJ::db::Database::new(&path).unwrap();

    let providers = ["anthropic", "openai", "deepseek", "gemini"];
    for (i, prov) in providers.iter().enumerate() {
        let mut rec = sample_record(&db);
        rec.id = Uuid::new_v4().to_string();
        rec.provider = prov.to_string();
        rec.input_tokens = 1000 * (i as u64 + 1);
        rec.cached_tokens = 800 * (i as u64 + 1);
        rec.actual_cost_cents = 0.10 * (i as f64 + 1.0);
        rec.saving_cents = 0.50 * (i as f64 + 1.0);
        db.insert_request(&rec).unwrap();
    }

    let stats = db.get_stats_since("1970-01-01").unwrap();
    assert_eq!(stats.total_requests, 4);
    assert!(stats.total_input_tokens > 0);
    assert!(stats.total_cached_tokens > 0);

    let recent = db.get_recent_requests(10).unwrap();
    assert_eq!(recent.len(), 4);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_db_zero_byte_file_recovery() {
    let path = temp_db_path();

    // 创建 0 字节文件模拟崩溃
    std::fs::write(&path, "").unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);

    // DB 应自动删除 0 字节文件并重建
    let db = TokenJ::db::Database::new(&path).unwrap();
    db.insert_request(&sample_record(&db)).unwrap();

    let stats = db.get_stats_since("1970-01-01").unwrap();
    assert_eq!(stats.total_requests, 1);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_db_time_filtered_stats() {
    let path = temp_db_path();
    let db = TokenJ::db::Database::new(&path).unwrap();

    // 插入一条记录
    db.insert_request(&sample_record(&db)).unwrap();

    // 查询未来的时间 → 0 条
    let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let stats = db.get_stats_since(&future).unwrap();
    assert_eq!(stats.total_requests, 0);

    // 查询过去的时间 → 1 条
    let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let stats = db.get_stats_since(&past).unwrap();
    assert_eq!(stats.total_requests, 1);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_db_concurrent_inserts() {
    use std::sync::Arc;
    use std::thread;

    let path = temp_db_path();
    let db = Arc::new(TokenJ::db::Database::new(&path).unwrap());

    let mut handles = vec![];
    for i in 0..10 {
        let db = db.clone();
        let handle = thread::spawn(move || {
            let mut rec = sample_record(&db);
            rec.id = Uuid::new_v4().to_string();
            rec.provider = format!("provider-{}", i);
            rec.input_tokens = (i * 100) as u64;
            db.insert_request(&rec).unwrap();
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = db.get_stats_since("1970-01-01").unwrap();
    assert_eq!(stats.total_requests, 10);

    let _ = std::fs::remove_file(&path);
}
