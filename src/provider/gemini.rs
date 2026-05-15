use crate::provider::{estimate_tokens, CacheInjection, CacheResult};
use serde_json::Value;

const MIN_CACHE_TOKENS: u64 = 32_768;

pub fn inject(body: &mut Value) -> CacheInjection {
    let mut details = Vec::new();

    let total_tokens = estimate_prompt_tokens(body);
    if total_tokens < MIN_CACHE_TOKENS {
        details.push(format!(
            "Gemini context too short for caching: ~{} tokens < {}",
            total_tokens, MIN_CACHE_TOKENS
        ));
        return CacheInjection { injected: false, details };
    }

    details.push(format!(
        "Gemini caching requires manual Context Cache API setup (~{} tokens detected)",
        total_tokens
    ));

    CacheInjection { injected: false, details }
}

pub fn parse_cache(_body: &Value) -> CacheResult {
    CacheResult {
        input_tokens: 0,
        output_tokens: 0,
        cached_tokens: 0,
        cache_write_tokens: 0,
    }
}

fn estimate_prompt_tokens(body: &Value) -> u64 {
    let mut total = 0u64;
    if let Some(contents) = body.get("contents").and_then(|v| v.as_array()) {
        for content in contents {
            if let Some(parts) = content.get("parts").and_then(|v| v.as_array()) {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        total += estimate_tokens(text);
                    }
                }
            }
        }
    }
    if let Some(system) = body.get("system_instruction").and_then(|v| v.as_str()) {
        total += estimate_tokens(system);
    }
    total
}
