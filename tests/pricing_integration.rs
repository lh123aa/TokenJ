use TokenJ::config::{Config, PriceConfig};

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
fn test_pricing_all_providers() {
    let cfg = test_config();

    let cases = [
        ("anthropic", "claude-sonnet-4-6", 5000, 200, 4500, 0),
        ("openai", "gpt-4o", 3000, 150, 2800, 0),
        ("deepseek", "deepseek-v4-flash", 1000, 50, 900, 0),
        ("gemini", "gemini-2.5-pro", 4000, 300, 3500, 0),
    ];

    for (provider, model, inp, out, cached, write) in &cases {
        let result = TokenJ::pricing::calculate_saving(
            provider, model, *inp, *out, *cached, *write, &cfg,
        );
        assert!(
            result.actual_cost_cents > 0.0,
            "{} {} should have positive cost",
            provider,
            model
        );
        assert!(
            result.saving_cents >= 0.0,
            "{} {} should have non-negative saving",
            provider,
            model
        );
        assert!(
            result.saving_rate >= 0.0,
            "{} {} should have non-negative saving rate",
            provider,
            model
        );
    }
}

#[test]
fn test_pricing_cache_write_vs_hit() {
    let cfg = test_config();

    // 缓存写入（首次请求）比无缓存更贵
    let write_cost = TokenJ::pricing::calculate_saving(
        "anthropic", "claude-sonnet-4-6", 5000, 0, 0, 5000, &cfg,
    );
    let no_cache_cost = TokenJ::pricing::calculate_saving(
        "anthropic", "claude-sonnet-4-6", 5000, 0, 0, 0, &cfg,
    );
    assert!(
        write_cost.actual_cost_cents > no_cache_cost.actual_cost_cents,
        "Cache write should cost more than no cache"
    );

    // 缓存命中比无缓存便宜
    let hit_cost = TokenJ::pricing::calculate_saving(
        "anthropic", "claude-sonnet-4-6", 5000, 0, 4500, 0, &cfg,
    );
    assert!(
        hit_cost.actual_cost_cents < no_cache_cost.actual_cost_cents,
        "Cache hit should cost less than no cache"
    );
}

#[test]
fn test_pricing_unknown_provider_fallback() {
    let cfg = test_config();

    let result = TokenJ::pricing::calculate_saving(
        "unknown-provider", "some-model", 1000, 100, 0, 0, &cfg,
    );

    // Fallback 价格应返回非零成本
    assert!(result.actual_cost_cents > 0.0, "Fallback should have cost");
    assert_eq!(result.saving_cents, 0.0, "No cache = no saving");
    assert_eq!(result.saving_rate, 0.0, "No cache = 0% saving rate");
}

#[test]
fn test_pricing_zero_tokens() {
    let cfg = test_config();

    let result = TokenJ::pricing::calculate_saving(
        "anthropic", "claude-sonnet-4-6", 0, 0, 0, 0, &cfg,
    );

    assert_eq!(result.actual_cost_cents, 0.0, "Zero tokens = zero cost");
    assert_eq!(result.saving_cents, 0.0);
    assert_eq!(result.saving_rate, 0.0);
}

#[test]
fn test_pricing_high_cache_hit_rate() {
    let cfg = test_config();

    let result = TokenJ::pricing::calculate_saving(
        "deepseek", "deepseek-v4-flash", 10000, 1000, 9000, 0, &cfg,
    );

    // 90% 缓存命中应节省超过 50%
    assert!(
        result.saving_rate > 50.0,
        "90% cache hit should save >50%, got {:.1}%",
        result.saving_rate
    );
}

#[test]
fn test_pricing_gemini_cache_hit() {
    let cfg = test_config();

    let result = TokenJ::pricing::calculate_saving(
        "gemini", "gemini-2.5-pro", 10000, 1000, 8000, 0, &cfg,
    );

    assert!(result.actual_cost_cents > 0.0, "Gemini should have cost");
    assert!(result.saving_cents > 0.0, "Gemini cache hit should save money");
}
