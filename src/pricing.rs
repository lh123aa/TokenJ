use crate::config::Config;

#[derive(Debug, Clone)]
pub struct CostBreakdown {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub actual_cost_cents: f64,
    pub saving_cents: f64,
    pub saving_rate: f64,
}

pub fn calculate_saving(
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: u64,
    cache_write_tokens: u64,
    config: &Config,
) -> CostBreakdown {
    let price = find_price(config, provider, model);

    let uncached_input = input_tokens.saturating_sub(cached_tokens);

    let actual_cost_cents = if cache_write_tokens > 0 {
        // First request: cache write cost
        let write_cost = (cache_write_tokens as f64 / 1_000_000.0) * price.cache_write_per_mtok;
        let uncached_cost = (uncached_input as f64 / 1_000_000.0) * price.input_per_mtok;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * price.output_per_mtok;
        (write_cost + uncached_cost + output_cost) * 100.0
    } else if cached_tokens > 0 {
        // Cache hit: read cost
        let read_cost = (cached_tokens as f64 / 1_000_000.0) * price.cache_read_per_mtok;
        let uncached_cost = (uncached_input as f64 / 1_000_000.0) * price.input_per_mtok;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * price.output_per_mtok;
        (read_cost + uncached_cost + output_cost) * 100.0
    } else {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * price.input_per_mtok;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * price.output_per_mtok;
        (input_cost + output_cost) * 100.0
    };

    // What it would cost without caching
    let no_cache_cost_cents = {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * price.input_per_mtok;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * price.output_per_mtok;
        (input_cost + output_cost) * 100.0
    };

    let saving_cents = no_cache_cost_cents - actual_cost_cents;
    let saving_rate = if no_cache_cost_cents > 0.0 {
        (saving_cents / no_cache_cost_cents) * 100.0
    } else {
        0.0
    };

    CostBreakdown {
        input_tokens,
        output_tokens,
        cached_tokens,
        cache_write_tokens,
        actual_cost_cents,
        saving_cents,
        saving_rate,
    }
}

fn find_price(config: &Config, provider: &str, model: &str) -> crate::config::ModelPrice {
    let model_lower = model.to_lowercase();
    let prices = match provider {
        "anthropic" => &config.prices.anthropic,
        "openai" => &config.prices.openai,
        "deepseek" => &config.prices.deepseek,
        _ => return crate::config::ModelPrice {
            model: model.into(),
            pattern: "".into(),
            input_per_mtok: 2.0,
            output_per_mtok: 8.0,
            cache_write_per_mtok: 2.0,
            cache_read_per_mtok: 2.0,
        },
    };

    for price in prices {
        if model_lower.contains(&price.pattern) {
            return price.clone();
        }
    }

    prices.first().cloned().unwrap_or(crate::config::ModelPrice {
        model: model.into(),
        pattern: "".into(),
        input_per_mtok: 2.0,
        output_per_mtok: 8.0,
        cache_write_per_mtok: 2.0,
        cache_read_per_mtok: 2.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PriceConfig};

    fn test_config() -> Config {
        Config {
            port: 9100,
            cert_dir: std::path::PathBuf::from("/tmp/certs"),
            data_dir: std::path::PathBuf::from("/tmp/data"),
            db_path: std::path::PathBuf::from("/tmp/data.db"),
            exclude_hosts: vec![],
            prices: PriceConfig::default(),
        }
    }

    #[test]
    fn test_claude_cache_hit_90_percent_saving() {
        let cfg = test_config();
        let result = calculate_saving("anthropic", "claude-sonnet-4-6", 5000, 200, 4500, 0, &cfg);
        // Cached tokens (4500) get 90% discount: $0.30 vs $3.00/MTok
        // But output tokens ($15/MTok) are not cached, so overall rate is lower
        assert!(result.saving_rate > 40.0, "Saving rate should be >40%, got {:.1}%", result.saving_rate);
        assert!(result.saving_cents > 0.0, "Should have saving");
    }

    #[test]
    fn test_claude_cache_full_hit_90_percent() {
        let cfg = test_config();
        // Only input tokens, no output → full 90% on cached portion
        let result = calculate_saving("anthropic", "claude-sonnet-4-6", 5000, 0, 4500, 0, &cfg);
        assert!(result.saving_rate > 80.0, "Saving rate should be ~90%, got {:.1}%", result.saving_rate);
    }

    #[test]
    fn test_claude_no_cache_no_saving() {
        let cfg = test_config();
        let result = calculate_saving("anthropic", "claude-sonnet-4-6", 5000, 200, 0, 0, &cfg);
        assert_eq!(result.saving_rate, 0.0, "No cache = no saving");
        assert_eq!(result.saving_cents, 0.0, "Saving should be 0");
    }

    #[test]
    fn test_claude_cache_write_cost_more() {
        let cfg = test_config();
        // First request writes cache → costs 125% of input
        let result = calculate_saving("anthropic", "claude-sonnet-4-6", 5000, 200, 0, 5000, &cfg);
        // Cache write is more expensive, so saving should be negative
        assert!(result.saving_cents < 0.0, "Cache write should cost more, got saving: {:.4}", result.saving_cents);
    }

    #[test]
    fn test_openai_cache_hit_75_percent_saving() {
        let cfg = test_config();
        let result = calculate_saving("openai", "gpt-4o", 3000, 150, 2800, 0, &cfg);
        assert!(result.saving_rate > 30.0, "Saving rate should be >30%, got {:.1}%", result.saving_rate);
    }

    #[test]
    fn test_deepseek_cache_hit_90_percent_saving() {
        let cfg = test_config();
        let result = calculate_saving("deepseek", "deepseek-v4-pro", 2000, 100, 1800, 0, &cfg);
        assert!(result.saving_rate > 30.0, "Saving rate should be >30%, got {:.1}%", result.saving_rate);
    }

    #[test]
    fn test_unknown_provider_fallback_price() {
        let cfg = test_config();
        let result = calculate_saving("unknown", "some-model", 1000, 100, 0, 0, &cfg);
        assert!(result.actual_cost_cents > 0.0, "Fallback price should produce cost");
    }

    #[test]
    fn test_zero_tokens_no_cost() {
        let cfg = test_config();
        let result = calculate_saving("anthropic", "claude-sonnet-4-6", 0, 0, 0, 0, &cfg);
        assert_eq!(result.actual_cost_cents, 0.0);
        assert_eq!(result.saving_cents, 0.0);
        assert_eq!(result.saving_rate, 0.0);
    }

    #[test]
    fn test_model_price_matching_by_pattern() {
        let cfg = test_config();
        // Should match "opus-4-7" pattern
        let r1 = calculate_saving("anthropic", "claude-opus-4-7", 1000, 100, 900, 0, &cfg);
        assert!(r1.saving_cents > 0.0);

        // Should match "haiku" pattern
        let r2 = calculate_saving("anthropic", "claude-haiku-4-5", 1000, 100, 0, 0, &cfg);
        assert!(r2.actual_cost_cents > 0.0);
    }

    #[test]
    fn test_high_cache_hit_rate() {
        let cfg = test_config();
        // 90% of input cached → should have high saving rate
        let result = calculate_saving("deepseek", "deepseek-v4-flash", 10000, 100, 9000, 0, &cfg);
        assert!(result.saving_rate > 50.0, "90% cache hit should save >50%");
    }

    #[test]
    fn test_cache_write_more_expensive_than_no_cache() {
        let cfg = test_config();
        // Cache write costs more than uncached input
        let result = calculate_saving("openai", "gpt-4o", 1000, 0, 0, 1000, &cfg);
        assert!(result.saving_cents < 0.0, "Cache write should cost more initially");
        assert!(result.saving_rate < 0.0, "Cache write should have negative saving rate");
    }

    #[test]
    fn test_mixed_cache_and_uncached() {
        let cfg = test_config();
        // Half cached, half not
        let result = calculate_saving("anthropic", "claude-sonnet-4-6", 2000, 100, 1000, 0, &cfg);
        assert!(result.saving_cents > 0.0, "Partial cache should still save");
        assert!(result.saving_rate > 20.0, "50% cache should save >20%");
    }

    #[test]
    fn test_deepseek_v4_flash_pricing() {
        let cfg = test_config();
        let result = calculate_saving("deepseek", "deepseek-v4-flash", 10000, 1000, 9000, 0, &cfg);
        assert!(result.actual_cost_cents > 0.0);
        assert!(result.saving_cents > 0.0);
        assert!(result.saving_rate > 50.0, "DeepSeek 90% cache hit should save >50%");
    }
}
