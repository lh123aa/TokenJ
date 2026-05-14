use crate::provider::{CacheInjection, CacheResult};
use serde_json::Value;

const MIN_CACHE_TOKENS: u64 = 1024;

pub fn inject(body: &mut Value) -> CacheInjection {
    let mut details = Vec::new();
    let mut injected = false;

    if let Some(system) = body.get_mut("system") {
        let system_tokens = estimate_tokens(system);
        if system_tokens >= MIN_CACHE_TOKENS {
            match system {
                Value::Array(arr) => {
                    if let Some(last) = arr.last_mut() {
                        if last.get("cache_control").is_none() {
                            if let Some(obj) = last.as_object_mut() {
                                let mut cc = serde_json::Map::new();
                                cc.insert("type".into(), Value::String("ephemeral".into()));
                                obj.insert("cache_control".into(), Value::Object(cc));
                                injected = true;
                                details.push(format!(
                                    "Added cache_control to system prompt (~{} tokens)",
                                    system_tokens
                                ));
                            }
                        }
                    }
                }
                Value::String(text) => {
                    let text = text.clone();
                    let mut cc = serde_json::Map::new();
                    cc.insert("type".into(), Value::String("ephemeral".into()));
                    let mut block = serde_json::Map::new();
                    block.insert("type".into(), Value::String("text".into()));
                    block.insert("text".into(), Value::String(text));
                    block.insert("cache_control".into(), Value::Object(cc));
                    body["system"] = Value::Array(vec![Value::Object(block)]);
                    injected = true;
                    details.push(format!(
                        "Converted system to array + cache_control (~{} tokens)",
                        system_tokens
                    ));
                }
                _ => {}
            }
        } else {
            details.push(format!(
                "System prompt too short for caching: {} tokens < {}",
                system_tokens, MIN_CACHE_TOKENS
            ));
        }
    }

    CacheInjection { injected, details }
}

pub fn parse_cache(body: &Value) -> CacheResult {
    let default_usage = serde_json::json!({});
    let usage = body.get("usage").unwrap_or(&default_usage);

    CacheResult {
        input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        cached_tokens: usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        cache_write_tokens: usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
    }
}

fn estimate_tokens(value: &Value) -> u64 {
    match value {
        Value::String(s) => (s.len() / 4) as u64,
        Value::Array(arr) => arr.iter().map(|v| estimate_tokens(v)).sum(),
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                (text.len() / 4) as u64
            } else {
                map.values().map(|v| estimate_tokens(v)).sum()
            }
        }
        _ => 0,
    }
}
