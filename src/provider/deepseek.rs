use crate::provider::{CacheInjection, CacheResult};
use serde_json::Value;

pub fn inject(_body: &mut Value) -> CacheInjection {
    CacheInjection {
        injected: false,
        details: vec!["DeepSeek caching is automatic, no injection needed".into()],
    }
}

pub fn parse_cache(body: &Value) -> CacheResult {
    let default_usage = serde_json::json!({});
    let usage = body.get("usage").unwrap_or(&default_usage);

    let prompt_details = usage.get("prompt_tokens_details").unwrap_or(&default_usage);

    let cached = prompt_details
        .get("cached_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    CacheResult {
        input_tokens: usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        output_tokens: usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        cached_tokens: cached,
        cache_write_tokens: 0,
    }
}
