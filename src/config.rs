use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub cert_dir: PathBuf,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub exclude_hosts: Vec<String>,
    pub prices: PriceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceConfig {
    pub anthropic: Vec<ModelPrice>,
    pub openai: Vec<ModelPrice>,
    pub deepseek: Vec<ModelPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub model: String,
    pub pattern: String,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_write_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs_data_dir();
        let cert_dir = data_dir.join("certs");
        let db_path = data_dir.join("data.db");

        Self {
            port: 9100,
            cert_dir,
            data_dir,
            db_path,
            exclude_hosts: vec![],
            prices: PriceConfig::default(),
        }
    }
}

impl PriceConfig {
    pub fn default() -> Self {
        Self {
            anthropic: vec![
                ModelPrice {
                    model: "claude-opus-4-7".into(),
                    pattern: "opus-4-7".into(),
                    input_per_mtok: 5.0,
                    output_per_mtok: 25.0,
                    cache_write_per_mtok: 6.25,
                    cache_read_per_mtok: 0.50,
                },
                ModelPrice {
                    model: "claude-opus-4-6".into(),
                    pattern: "opus-4-6".into(),
                    input_per_mtok: 5.0,
                    output_per_mtok: 25.0,
                    cache_write_per_mtok: 6.25,
                    cache_read_per_mtok: 0.50,
                },
                ModelPrice {
                    model: "claude-sonnet-4-6".into(),
                    pattern: "sonnet-4-6".into(),
                    input_per_mtok: 3.0,
                    output_per_mtok: 15.0,
                    cache_write_per_mtok: 3.75,
                    cache_read_per_mtok: 0.30,
                },
                ModelPrice {
                    model: "claude-haiku-4-5".into(),
                    pattern: "haiku".into(),
                    input_per_mtok: 1.0,
                    output_per_mtok: 5.0,
                    cache_write_per_mtok: 1.25,
                    cache_read_per_mtok: 0.10,
                },
            ],
            openai: vec![
                ModelPrice {
                    model: "gpt-4o".into(),
                    pattern: "gpt-4o".into(),
                    input_per_mtok: 2.50,
                    output_per_mtok: 10.0,
                    cache_write_per_mtok: 2.50,
                    cache_read_per_mtok: 0.625,
                },
                ModelPrice {
                    model: "gpt-4o-mini".into(),
                    pattern: "gpt-4o-mini".into(),
                    input_per_mtok: 0.15,
                    output_per_mtok: 0.60,
                    cache_write_per_mtok: 0.15,
                    cache_read_per_mtok: 0.0375,
                },
            ],
            deepseek: vec![
                ModelPrice {
                    model: "deepseek-v4-pro".into(),
                    pattern: "v4-pro".into(),
                    input_per_mtok: 1.74,
                    output_per_mtok: 3.48,
                    cache_write_per_mtok: 1.74,
                    cache_read_per_mtok: 0.145,
                },
                ModelPrice {
                    model: "deepseek-v4-flash".into(),
                    pattern: "v4-flash".into(),
                    input_per_mtok: 0.14,
                    output_per_mtok: 0.28,
                    cache_write_per_mtok: 0.14,
                    cache_read_per_mtok: 0.028,
                },
            ],
        }
    }
}

fn dirs_data_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "tokenj", "tokenJ") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".tokenj")
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let data_dir = dirs_data_dir();
        let config_path = data_dir.join("config.json");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut cfg: Config = serde_json::from_str(&content)?;
            cfg.data_dir = data_dir.clone();
            cfg.cert_dir = data_dir.join("certs");
            cfg.db_path = data_dir.join("data.db");
            Ok(cfg)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let config_path = self.data_dir.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }
}
