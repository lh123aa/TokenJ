use crate::provider::{CacheInjection, CacheResult};
use serde_json::Value;

const MIN_CACHE_TOKENS: u64 = 1024;

pub fn inject(body: &mut Value) -> CacheInjection {
    let mut details = Vec::new();
    let mut injected = false;

    // Estimate total prompt tokens
    let total_tokens = estimate_prompt_tokens(body);

    if total_tokens < MIN_CACHE_TOKENS {
        details.push(format!("Prompt too short for caching: {} tokens < {}", total_tokens, MIN_CACHE_TOKENS));
        return CacheInjection { injected: false, details };
    }

    // Check if prompt_cache_key already set
    if body.get("prompt_cache_key").is_some() {
        details.push("prompt_cache_key already set, skipping".into());
        return CacheInjection { injected: false, details };
    }

    // Generate prompt_cache_key from system prompt hash
    if let Some(key) = generate_cache_key(body) {
        body["prompt_cache_key"] = Value::String(key);
        injected = true;
        details.push(format!("Added prompt_cache_key (~{} tokens, auto)", total_tokens));
    }

    CacheInjection { injected, details }
}

pub fn parse_cache(body: &Value) -> CacheResult {
    let default_usage = serde_json::json!({});
    let usage = body.get("usage").unwrap_or(&default_usage);

    let prompt_details = usage.get("prompt_tokens_details").unwrap_or(&default_usage);

    CacheResult {
        input_tokens: usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        output_tokens: usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        cached_tokens: prompt_details.get("cached_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        cache_write_tokens: 0,
    }
}

fn estimate_prompt_tokens(body: &Value) -> u64 {
    let mut total = 0u64;

    if let Some(messages) = body.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            if let Some(content) = msg.get("content") {
                total += estimate_content_tokens(content);
            }
        }
    }

    if let Some(system) = body.get("system").and_then(|v| v.as_str()) {
        total += (system.len() / 4) as u64;
    }

    total
}

fn estimate_content_tokens(value: &Value) -> u64 {
    match value {
        Value::String(s) => (s.len() / 4) as u64,
        Value::Array(arr) => arr.iter().map(|v| estimate_content_tokens(v)).sum(),
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                (text.len() / 4) as u64
            } else {
                0
            }
        }
        _ => 0,
    }
}

fn generate_cache_key(body: &Value) -> Option<String> {
    use sha2::{Digest, Sha256};

    let system_content = body
        .get("messages")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|msg| msg.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if system_content.len() < 100 {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(system_content.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = hex::encode(&hash[..4]);

    Some(format!("tokenj-{}", hash_hex))
}
