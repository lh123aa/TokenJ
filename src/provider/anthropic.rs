use crate::provider::{estimate_tokens, CacheInjection, CacheResult};
use serde_json::Value;

const MIN_CACHE_TOKENS: u64 = 1024;

pub fn inject(body: &mut Value) -> CacheInjection {
    let mut details = Vec::new();
    let mut injected = false;

    if let Some(system) = body.get_mut("system") {
        let system_tokens = estimate_tokens_value(system);
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

fn estimate_tokens_value(value: &Value) -> u64 {
    match value {
        Value::String(s) => estimate_tokens(s),
        Value::Array(arr) => arr.iter().map(|v| estimate_tokens_value(v)).sum(),
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                estimate_tokens(text)
            } else {
                map.values().map(|v| estimate_tokens_value(v)).sum()
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_inject_on_long_system_string() {
        let mut body = json!({
            "model": "claude-opus-4-7",
            "system": "A".repeat(5000),
            "messages": [{"role": "user", "content": "hi"}]
        });
        let result = inject(&mut body);
        assert!(result.injected, "Should inject cache_control on long system prompt");
        assert!(body["system"].is_array(), "System should be converted to array");
        assert!(body["system"][0].get("cache_control").is_some(), "cache_control should be present");
    }

    #[test]
    fn test_no_inject_on_short_system() {
        let mut body = json!({
            "model": "claude-opus-4-7",
            "system": "Short",
            "messages": [{"role": "user", "content": "hi"}]
        });
        let result = inject(&mut body);
        assert!(!result.injected, "Should NOT inject on short system prompt");
    }

    #[test]
    fn test_no_inject_if_already_has_cache_control() {
        let mut body = json!({
            "model": "claude-opus-4-7",
            "system": [
                {"type": "text", "text": "A".repeat(2000), "cache_control": {"type": "ephemeral"}}
            ],
            "messages": [{"role": "user", "content": "hi"}]
        });
        let result = inject(&mut body);
        assert!(!result.injected, "Should NOT override existing cache_control");
    }

    #[test]
    fn test_inject_on_long_array_system() {
        let mut body = json!({
            "model": "claude-opus-4-7",
            "system": [
                {"type": "text", "text": "A".repeat(3000)},
                {"type": "text", "text": "B".repeat(3000)}
            ],
            "messages": [{"role": "user", "content": "hi"}]
        });
        let result = inject(&mut body);
        assert!(result.injected, "Should inject on long array system prompt");
    }

    #[test]
    fn test_parse_cache_with_hit() {
        let body = json!({
            "usage": {
                "input_tokens": 5200,
                "output_tokens": 200,
                "cache_read_input_tokens": 5000,
                "cache_creation_input_tokens": 0
            }
        });
        let result = parse_cache(&body);
        assert_eq!(result.input_tokens, 5200);
        assert_eq!(result.cached_tokens, 5000);
        assert_eq!(result.cache_write_tokens, 0);
    }

    #[test]
    fn test_parse_cache_with_write() {
        let body = json!({
            "usage": {
                "input_tokens": 5200,
                "output_tokens": 200,
                "cache_read_input_tokens": 0,
                "cache_creation_input_tokens": 5000
            }
        });
        let result = parse_cache(&body);
        assert_eq!(result.cache_write_tokens, 5000);
        assert_eq!(result.cached_tokens, 0);
    }

    #[test]
    fn test_parse_cache_no_cache() {
        let body = json!({
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });
        let result = parse_cache(&body);
        assert_eq!(result.cached_tokens, 0);
        assert_eq!(result.cache_write_tokens, 0);
    }

    #[test]
    fn test_no_system_field_no_injection() {
        let mut body = json!({
            "model": "claude-opus-4-7",
            "messages": [{"role": "user", "content": "hi"}]
        });
        let result = inject(&mut body);
        assert!(!result.injected);
    }
}
