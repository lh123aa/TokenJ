use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use TokenJ::cert::CertManager;
use TokenJ::config::Config;
use TokenJ::dashboard;
use TokenJ::db::Database;
use TokenJ::proxy::{Proxy, ProxyEvent};
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "TokenJ", version = "0.2.0", about = "Automatic LLM API cache optimizer - save up to 90% on API costs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the proxy (direct mode: set base_url in SDK)
    Proxy {
        /// Listen port
        #[arg(short, long, default_value = "9100")]
        port: u16,
    },
    /// Start the TUI dashboard (use --json for non-interactive mode)
    Dashboard {
        /// Output JSON stats to stdout instead of launching TUI
        #[arg(long)]
        json: bool,
    },
    /// Run demo with sample data
    Demo,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new("info").unwrap_or(EnvFilter::new("info")))
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Proxy { port } => run_proxy(port).await,
        Commands::Dashboard { json } => {
            if json {
                run_dashboard_json().await
            } else {
                run_dashboard_mode().await
            }
        }
        Commands::Demo => run_demo().await,
    }
}

async fn run_proxy(port: u16) -> Result<()> {
    // 加载配置，并用 CLI 传入的端口覆盖
    let mut cfg = Config::load()?;
    cfg.port = port;
    let config = Arc::new(cfg);

    // 导出价格表供 Python MCP Server 使用
    let _ = config.export_prices_json();

    // Ensure cert directory exists and generate CA cert
    let cert_manager = Arc::new(CertManager::load_or_create(&config.cert_dir)?);
    let _ = cert_manager.ca_cert_pem()?;

    println!();
    println!("  ╔══════════════════════════════════════════════╗");
    println!("  ║        TokenJ — 自动缓存优化引擎            ║");
    println!("  ║        装了就省钱 · 零配置                   ║");
    println!("  ║        当前模式: 直连模式                    ║");
    println!("  ╚══════════════════════════════════════════════╝");
    println!();

    // Setup database
    let db = Arc::new(Database::new(&config.db_path)?);

    // Setup event channel for real-time updates
    let (event_tx, _) = broadcast::channel::<ProxyEvent>(256);

    // Start proxy
    let proxy = Proxy::new(config.clone(), db.clone(), event_tx.clone(), cert_manager.clone());
    let running = Arc::new(AtomicBool::new(true));

    println!("  代理运行在: http://127.0.0.1:{}", port);
    println!();
    println!("  📋 快速开始:");
    println!();
    println!("  方式 A — 直连模式（推荐，支持缓存注入 ✅）:");
    println!("    修改 LLM SDK 的 base_url 指向本代理:");
    println!("    OpenAI:    client = OpenAI(base_url=\"http://127.0.0.1:{}/v1\")", port);
    println!("    Anthropic: client = Anthropic(base_url=\"http://127.0.0.1:{}\")", port);
    println!("    DeepSeek:  client = DeepSeek(base_url=\"http://127.0.0.1:{}\")", port);
    println!();
    println!("  方式 B — HTTPS_PROXY 模式（LLM 域名自动 MITM ✅ 非 LLM 透传）:");
    println!("    export HTTPS_PROXY=http://127.0.0.1:{}", port);
    println!("    LLM 域名(anthropic/openai/deepseek等)→自动 TLS 解密+缓存注入");
    println!("    其他域名→透传隧道（不干预）");
    println!();
    println!("  💡 首次使用方式 B 需安装 CA 证书（见上方指引）");
    println!();
    println!("  📊 打开仪表盘: TokenJ dashboard");
    println!("  🛑 按 Ctrl+C 停止代理");
    println!();

    // 检查 CA 证书是否存在，引导用户安装（方式 B 需要）
    let ca_path = config.cert_dir.join("ca.crt");
    if ca_path.exists() {
        println!("  🔐 CA 证书路径: {}", ca_path.display());
        println!("  📖 方式 B 需要安装此证书到系统信任存储:");
        println!("     Windows: 双击 ca.crt → 安装到「受信任的根证书颁发机构」");
        println!("     macOS:    sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {}", ca_path.display());
        println!("     Linux:    sudo cp {} /usr/local/share/ca-certificates/ && sudo update-ca-certificates", ca_path.display());
        println!();
    }

    // Start dashboard in background if possible
    let db_bg = db.clone();
    let running_bg = running.clone();
    let _event_rx = event_tx.subscribe();

    tokio::spawn(async move {
        // Brief delay to let proxy start, then show stats periodically
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            if !running_bg.load(Ordering::Relaxed) {
                break;
            }
            if let Ok(stats) = db_bg.get_stats_since("1970-01-01") {
                let cost = stats.total_cost_cents / 100.0;
                let saving = stats.total_saving_cents / 100.0;
                println!(
                    "  [TokenJ] 请求: {} | 成本: ${:.2} | 节省: ${:.2} | 命中率: {:.1}%",
                    stats.total_requests, cost, saving, stats.cache_hit_rate
                );
            }
        }
    });

    // Run proxy (blocking)
    proxy.run().await?;

    Ok(())
}

async fn run_dashboard_mode() -> Result<()> {
    let config = Arc::new(Config::load()?);
    let _ = config.export_prices_json();
    let db = Arc::new(Database::new(&config.db_path)?);
    let (_event_tx, event_rx) = broadcast::channel::<ProxyEvent>(256);
    let running = Arc::new(AtomicBool::new(true));

    dashboard::run_dashboard(db, event_rx, running).await
}

/// --json 模式：不启动 TUI，直接输出 JSON 统计到 stdout
async fn run_dashboard_json() -> Result<()> {
    let config = Arc::new(Config::load()?);
    let db = Arc::new(Database::new(&config.db_path)?);

    let stats = db.get_stats_since("1970-01-01")?;
    let output = serde_json::json!({
        "total_requests": stats.total_requests,
        "total_cost_dollars": (stats.total_cost_cents / 100.0 * 100.0).round() / 100.0,
        "total_saving_dollars": (stats.total_saving_cents / 100.0 * 100.0).round() / 100.0,
        "cache_hit_rate": (stats.cache_hit_rate * 100.0).round() / 100.0,
        "avg_saving_rate": (stats.avg_saving_rate * 100.0).round() / 100.0,
        "total_input_tokens": stats.total_input_tokens,
        "total_output_tokens": stats.total_output_tokens,
        "total_cached_tokens": stats.total_cached_tokens,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

async fn run_demo() -> Result<()> {
    let config = Arc::new(Config::load()?);
    // 导出价格表供 Python MCP Server 使用
    let _ = config.export_prices_json();
    let db = Arc::new(Database::new(&config.db_path)?);
    let (event_tx, event_rx) = broadcast::channel::<ProxyEvent>(256);
    let running = Arc::new(AtomicBool::new(true));

    // Insert sample data
    let sample_data = vec![
        ("anthropic", "claude-sonnet-4-6", 5000, 200, 4500, 0, 0.30, 90.0),
        ("openai", "gpt-4o", 3000, 150, 0, 3000, 0.015, 0.0),
        ("anthropic", "claude-opus-4-7", 8000, 400, 7500, 0, 0.40, 93.0),
        ("deepseek", "deepseek-v4-pro", 2000, 100, 1800, 0, 0.013, 90.0),
        ("anthropic", "claude-sonnet-4-6", 5000, 250, 4800, 0, 0.31, 91.0),
        ("openai", "gpt-4o-mini", 1500, 80, 0, 0, 0.003, 0.0),
        ("anthropic", "claude-haiku-4-5", 2000, 100, 0, 2000, 0.007, 0.0),
        ("deepseek", "deepseek-v4-flash", 1000, 50, 900, 0, 0.002, 90.0),
        ("anthropic", "claude-opus-4-7", 8000, 500, 7600, 0, 0.42, 94.0),
        ("openai", "gpt-4o", 3000, 200, 2800, 0, 0.02, 85.0),
    ];

    for (provider, model, input, output, cached, write, cost, rate) in &sample_data {
        let rec = TokenJ::db::RequestRecord {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "demo".into(),
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens: *input,
            output_tokens: *output,
            cached_tokens: *cached,
            cache_write_tokens: *write,
            actual_cost_cents: *cost,
            saving_cents: *cost * *rate / 100.0,
            saving_rate: *rate,
            cache_injected: true,
            duration_ms: 500,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let _ = db.insert_request(&rec);

        let _ = event_tx.send(ProxyEvent {
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens: *input,
            output_tokens: *output,
            cached_tokens: *cached,
            cache_write_tokens: *write,
            saving_cents: *cost * *rate / 100.0,
            saving_rate: *rate,
            cache_injected: true,
            duration_ms: 500,
        });
    }

    println!("  Loaded demo data with {} sample requests", sample_data.len());
    println!("  Opening dashboard...\n");

    dashboard::run_dashboard(db, event_rx, running).await
}
