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
