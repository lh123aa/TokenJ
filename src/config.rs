use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    #[serde(default)]
    pub gemini: Vec<ModelPrice>,
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

/// 外部 prices.json 中的扁平化价格条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatPriceEntry {
    pub key: String,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_write_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

impl PriceConfig {
    /// 从外部 prices.json 加载价格表；若文件不存在或格式错误则回退到编译期默认值
    pub fn load(prices_path: &Path) -> Self {
        if prices_path.exists() {
            match std::fs::read_to_string(prices_path) {
                Ok(content) => {
                    if let Ok(entries) = serde_json::from_str::<Vec<FlatPriceEntry>>(&content) {
                    return Self::from_flat_entries(&entries);
                    }
                }
                Err(_) => {}
            }
        }
        Self::default()
    }

    /// 从扁平化价格条目构建 PriceConfig（清空默认值，完全从文件加载）
    fn from_flat_entries(entries: &[FlatPriceEntry]) -> Self {
        let mut config = Self {
            anthropic: Vec::new(),
            openai: Vec::new(),
            deepseek: Vec::new(),
            gemini: Vec::new(),
        };
        for entry in entries {
            let parts: Vec<&str> = entry.key.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }
            let provider = parts[0];
            let pattern = parts[1];
            let mp = ModelPrice {
                model: pattern.to_string(),
                pattern: pattern.to_string(),
                input_per_mtok: entry.input_per_mtok,
                output_per_mtok: entry.output_per_mtok,
                cache_write_per_mtok: entry.cache_write_per_mtok,
                cache_read_per_mtok: entry.cache_read_per_mtok,
            };
            match provider {
                "anthropic" => config.anthropic.push(mp),
                "openai" => config.openai.push(mp),
                "deepseek" => config.deepseek.push(mp),
                "gemini" => config.gemini.push(mp),
                _ => {}
            }
        }
        config
    }

    /// 导出为扁平化价格条目列表（供 Python MCP Server 和外部 prices.json 使用）
    pub fn to_flat_entries(&self) -> Vec<FlatPriceEntry> {
        let mut entries = Vec::new();
        let provider_groups: [(&str, &[ModelPrice]); 4] = [
            ("anthropic", &self.anthropic),
            ("openai", &self.openai),
            ("deepseek", &self.deepseek),
            ("gemini", &self.gemini),
        ];
        for (provider, prices) in provider_groups.into_iter() {
            for p in prices {
                entries.push(FlatPriceEntry {
                    key: format!("{}:{}", provider, p.pattern),
                    input_per_mtok: p.input_per_mtok,
                    output_per_mtok: p.output_per_mtok,
                    cache_write_per_mtok: p.cache_write_per_mtok,
                    cache_read_per_mtok: p.cache_read_per_mtok,
                });
            }
        }
        entries
    }

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
            gemini: vec![
                ModelPrice {
                    model: "gemini-2.5-pro".into(),
                    pattern: "gemini-2.5-pro".into(),
                    input_per_mtok: 1.25,
                    output_per_mtok: 5.0,
                    cache_write_per_mtok: 1.25,
                    cache_read_per_mtok: 0.3125,
                },
                ModelPrice {
                    model: "gemini-2.5-flash".into(),
                    pattern: "gemini-2.5-flash".into(),
                    input_per_mtok: 0.15,
                    output_per_mtok: 0.60,
                    cache_write_per_mtok: 0.15,
                    cache_read_per_mtok: 0.0375,
                },
                ModelPrice {
                    model: "gemini-2.0-flash".into(),
                    pattern: "gemini-2.0-flash".into(),
                    input_per_mtok: 0.10,
                    output_per_mtok: 0.40,
                    cache_write_per_mtok: 0.10,
                    cache_read_per_mtok: 0.025,
                },
            ],
        }
    }
}

fn dirs_data_dir() -> PathBuf {
    // 统一使用 ~/.TokenJ 目录，与 Python MCP Server 保持一致
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".TokenJ")
}

impl Config {
    pub fn load() -> Result<Self> {
        let data_dir = dirs_data_dir();
        let config_path = data_dir.join("config.json");
        let prices_path = data_dir.join("prices.json");

        let mut cfg = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut c: Config = serde_json::from_str(&content)?;
            c.data_dir = data_dir.clone();
            c.cert_dir = data_dir.join("certs");
            c.db_path = data_dir.join("data.db");
            c
        } else {
            Config::default()
        };

        // 从外部 prices.json 加载价格（若存在），覆盖编译期默认值
        cfg.prices = PriceConfig::load(&prices_path);

        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let config_path = self.data_dir.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    /// 导出 Python MCP Server 可读取的扁平化价格表（也作为 Rust 端的持久化来源）
    pub fn export_prices_json(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)
            .context("Failed to create data directory")?;
        let prices_path = self.data_dir.join("prices.json");
        let entries = self.prices.to_flat_entries();
        let content = serde_json::to_string_pretty(&entries)
            .context("Failed to serialize prices")?;
        std::fs::write(&prices_path, content)
            .context("Failed to write prices.json")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_port() {
        let cfg = Config::default();
        assert_eq!(cfg.port, 9100);
    }

    #[test]
    fn test_config_default_exclude_hosts_empty() {
        let cfg = Config::default();
        assert!(cfg.exclude_hosts.is_empty());
    }

    #[test]
    fn test_price_config_default_has_anthropic() {
        let prices = PriceConfig::default();
        assert!(!prices.anthropic.is_empty());
        assert!(prices.anthropic.iter().any(|p| p.pattern.contains("sonnet")));
    }

    #[test]
    fn test_price_config_default_has_openai() {
        let prices = PriceConfig::default();
        assert!(!prices.openai.is_empty());
        assert!(prices.openai.iter().any(|p| p.pattern.contains("gpt-4o")));
    }

    #[test]
    fn test_price_config_default_has_deepseek() {
        let prices = PriceConfig::default();
        assert!(!prices.deepseek.is_empty());
        assert!(prices.deepseek.iter().any(|p| p.pattern.contains("v4")));
    }

    #[test]
    fn test_price_config_default_has_gemini() {
        let prices = PriceConfig::default();
        assert!(!prices.gemini.is_empty());
        assert!(prices.gemini.iter().any(|p| p.pattern.contains("gemini")));
    }

    #[test]
    fn test_price_config_load_from_external_json() {
        let dir = std::env::temp_dir().join(format!("TokenJ_cfg_load_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let prices_path = dir.join("prices.json");

        // 先写入外部价格表
        let external = vec![
            crate::config::FlatPriceEntry {
                key: "anthropic:custom-model".into(),
                input_per_mtok: 9.99,
                output_per_mtok: 19.99,
                cache_write_per_mtok: 5.0,
                cache_read_per_mtok: 1.0,
            },
            crate::config::FlatPriceEntry {
                key: "gemini:gemini-2.5-pro".into(),
                input_per_mtok: 2.0,
                output_per_mtok: 8.0,
                cache_write_per_mtok: 2.0,
                cache_read_per_mtok: 0.5,
            },
        ];
        let content = serde_json::to_string_pretty(&external).unwrap();
        std::fs::write(&prices_path, content).unwrap();

        // 加载，验证外部条目合并到了默认值中
        let prices = PriceConfig::load(&prices_path);
        assert!(prices.anthropic.iter().any(|p| p.pattern == "custom-model"), "Should load external anthropic price");
        assert!(prices.gemini.iter().any(|p| p.pattern == "gemini-2.5-pro"), "Should load external gemini price");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_export_prices_json_format() {
        let dir = std::env::temp_dir().join(format!("TokenJ_cfg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let cfg = Config {
            data_dir: dir.clone(),
            prices: PriceConfig::default(),
            ..Config::default()
        };
        cfg.export_prices_json().unwrap();

        let prices_path = dir.join("prices.json");
        assert!(prices_path.exists(), "prices.json should be created");

        let content = std::fs::read_to_string(&prices_path).unwrap();
        let entries: Vec<FlatPriceEntry> = serde_json::from_str(&content).unwrap();
        assert!(!entries.is_empty(), "Should have at least one price entry");

        // Check that all entries have required fields
        for entry in &entries {
            assert!(!entry.key.is_empty());
            assert!(entry.input_per_mtok > 0.0);
            assert!(entry.output_per_mtok > 0.0);
            assert!(entry.cache_read_per_mtok >= 0.0);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_export_prices_contains_all_providers() {
        let dir = std::env::temp_dir().join(format!("TokenJ_cfg_prov_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let cfg = Config {
            data_dir: dir.clone(),
            prices: PriceConfig::default(),
            ..Config::default()
        };
        cfg.export_prices_json().unwrap();

        let prices_path = dir.join("prices.json");
        let content = std::fs::read_to_string(&prices_path).unwrap();
        let entries: Vec<FlatPriceEntry> = serde_json::from_str(&content).unwrap();

        let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        assert!(keys.iter().any(|k| k.starts_with("anthropic:")), "Should have anthropic prices");
        assert!(keys.iter().any(|k| k.starts_with("openai:")), "Should have openai prices");
        assert!(keys.iter().any(|k| k.starts_with("deepseek:")), "Should have deepseek prices");
        assert!(keys.iter().any(|k| k.starts_with("gemini:")), "Should have gemini prices");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
