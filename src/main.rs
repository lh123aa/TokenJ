use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokenj::cert::CertManager;
use tokenj::config::Config;
use tokenj::dashboard;
use tokenj::db::Database;
use tokenj::proxy::{Proxy, ProxyEvent};
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "tokenJ", version = "0.1.0", about = "Automatic LLM API cache optimizer - save up to 90% on API costs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MITM proxy
    Proxy {
        /// Listen port
        #[arg(short, long, default_value = "9100")]
        port: u16,
    },
    /// Start the TUI dashboard
    Dashboard,
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
        Commands::Dashboard => run_dashboard_mode().await,
        Commands::Demo => run_demo().await,
    }
}

async fn run_proxy(port: u16) -> Result<()> {
    let config = Arc::new(Config::load()?);

    // Ensure cert directory exists and generate CA cert
    let cert_manager = Arc::new(CertManager::load_or_create(&config.cert_dir)?);
    let _ = cert_manager.ca_cert_pem()?;

    println!();
    println!("  ╔══════════════════════════════════════════════╗");
    println!("  ║        tokenJ — 自动缓存优化引擎            ║");
    println!("  ║        装了就省钱，零配置                    ║");
    println!("  ╚══════════════════════════════════════════════╝");
    println!();

    // Setup database
    let db = Arc::new(Database::new(&config.db_path)?);

    // Setup event channel for real-time updates
    let (event_tx, _) = broadcast::channel::<ProxyEvent>(256);

    // Start proxy
    let proxy = Proxy::new(config.clone(), db.clone(), event_tx.clone());
    let running = Arc::new(AtomicBool::new(true));

    println!("  Proxy running on: http://127.0.0.1:{}", port);
    println!();
    println!("  Quick start:");
    println!("    1. Install CA certificate:");
    println!("       Windows: 双击 {}ca.crt → 安装到受信任的根证书颁发机构", cert_manager.cert_dir().display());
    println!("       macOS:   sudo security add-trusted-cert -d -r trustRoot -k \\");
    println!("                /Library/Keychains/System.keychain {}", cert_manager.cert_dir().display());
    println!("                ca.crt");
    println!();
    println!("    2. Set environment variable:");
    println!("       export HTTPS_PROXY=http://127.0.0.1:{}", port);
    println!();
    println!("    3. Start using LLM APIs normally!");
    println!("    4. Open dashboard: tokenJ dashboard");
    println!();

    // Update config with actual port
    let mut cfg = Config::load()?;
    if cfg.port != port {
        cfg.port = port;
        let _ = cfg.save();
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
                    "  [tokenJ] 请求: {} | 成本: ${:.2} | 节省: ${:.2} | 命中率: {:.1}%",
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
    let db = Arc::new(Database::new(&config.db_path)?);
    let (_event_tx, event_rx) = broadcast::channel::<ProxyEvent>(256);
    let running = Arc::new(AtomicBool::new(true));

    dashboard::run_dashboard(db, event_rx, running).await
}

async fn run_demo() -> Result<()> {
    let config = Arc::new(Config::load()?);
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
        let rec = tokenj::db::RequestRecord {
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
