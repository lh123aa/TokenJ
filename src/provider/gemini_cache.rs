//! Gemini Context Cache API 集成
//!
//! Gemini 的上下文缓存需要手动创建 `CachedContent`，然后通过 `cachedContent` 字段引用。
//! 流程：
//! 1. 拦截请求 → 计算内容哈希 → 查本地缓存
//! 2. 缓存命中 → 注入 `cachedContent` 字段
//! 3. 缓存未命中 → 首次请求透传，异步创建缓存供下次使用

use crate::provider::{estimate_tokens, CacheInjection};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

const MIN_CACHE_TOKENS: u64 = 32_768;

struct CacheEntry {
    cache_name: String,
}

/// Gemini 上下文缓存管理器
pub struct GeminiCacheStore {
    caches: Mutex<HashMap<String, CacheEntry>>,
}

impl GeminiCacheStore {
    pub fn new() -> Self {
        Self {
            caches: Mutex::new(HashMap::new()),
        }
    }

    /// 计算请求体内容的缓存哈希
    pub fn compute_hash(body: &Value) -> String {
        let mut hasher = Sha256::new();
        if let Some(contents) = body.get("contents") {
            hasher.update(contents.to_string().as_bytes());
        }
        if let Some(system) = body.get("system_instruction") {
            hasher.update(system.to_string().as_bytes());
        }
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// 查找现有缓存
    pub fn get_cache(&self, hash: &str) -> Option<String> {
        let caches = self.caches.lock().expect("gemini cache poisoned");
        caches.get(hash).map(|e| e.cache_name.clone())
    }

    /// 存储缓存
    pub fn set_cache(&self, hash: String, cache_name: String) {
        let mut caches = self.caches.lock().expect("gemini cache poisoned");
        caches.insert(hash, CacheEntry { cache_name });
    }

    /// 从 URL 中提取 API Key
    pub fn extract_api_key(url: &str) -> Option<String> {
        // 格式: /v1beta/models/...?key=API_KEY 或 ...=...&key=API_KEY
        for part in url.split('?').nth(1).unwrap_or("").split('&') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() == 2 && kv[0] == "key" && !kv[1].is_empty() {
                return Some(kv[1].to_string());
            }
        }
        None
    }
}

/// 处理 Gemini 请求：检测缓存条件、注入缓存字段
pub fn handle_gemini_request(
    body: &mut Value,
    api_key: &str,
    model: &str,
    cache_store: &GeminiCacheStore,
) -> CacheInjection {
    // 检查内容是否足够长
    let total_tokens = estimate_prompt_tokens(body);
    if total_tokens < MIN_CACHE_TOKENS {
        return CacheInjection {
            injected: false,
            details: vec![format!(
                "Gemini context too short: ~{} tokens < {}",
                total_tokens, MIN_CACHE_TOKENS
            )],
        };
    }

    // 计算内容哈希，查缓存
    let content_hash = GeminiCacheStore::compute_hash(body);
    if let Some(cached_name) = cache_store.get_cache(&content_hash) {
        // 缓存命中 → 注入 cachedContent
        body["cachedContent"] = Value::String(cached_name);
        // 注入后移除 contents（cachedContent 已包含它们）
        body.as_object_mut().map(|o| o.remove("contents"));
        body.as_object_mut().map(|o| o.remove("system_instruction"));

        return CacheInjection {
            injected: true,
            details: vec![format!(
                "Injected cachedContent (hash={}, ~{} tokens)",
                &content_hash[..8],
                total_tokens
            )],
        };
    }

    // 缓存未命中 → 尝试创建缓存（通过 HTTP 调用）
    match create_cached_content(body, api_key, model) {
        Ok(cache_name) => {
            cache_store.set_cache(content_hash.clone(), cache_name.clone());
            // 首次创建后，当前请求仍然使用缓存
            body["cachedContent"] = Value::String(cache_name);
            body.as_object_mut().map(|o| o.remove("contents"));
            body.as_object_mut().map(|o| o.remove("system_instruction"));

            CacheInjection {
                injected: true,
                details: vec![format!(
                    "Created Gemini Context Cache (hash={}, ~{} tokens)",
                    &content_hash[..8],
                    total_tokens
                )],
            }
        }
        Err(e) => CacheInjection {
            injected: false,
            details: vec![format!(
                "Failed to create Gemini cache for hash={}: {}",
                &content_hash[..8], e
            )],
        },
    }
}

/// 调用 Gemini Context Cache API 创建缓存
fn create_cached_content(body: &Value, api_key: &str, model: &str) -> Result<String, String> {
    // 构建缓存的请求体
    let mut cache_body = serde_json::Map::new();

    // 模型名
    let model_name = if model.starts_with("models/") {
        model.to_string()
    } else {
        format!("models/{}", model)
    };
    cache_body.insert("model".into(), Value::String(model_name));

    // 内容
    if let Some(contents) = body.get("contents") {
        cache_body.insert("contents".into(), contents.clone());
    }
    if let Some(system) = body.get("system_instruction") {
        cache_body.insert("systemInstruction".into(), system.clone());
    }

    // 设置 TTL (1小时)
    cache_body.insert("ttl".into(), Value::String("3600s".into()));

    let request_body = Value::Object(cache_body);
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/cachedContents?key={}",
        api_key
    );

    // 同步 HTTP 请求（在 async 上下文中由调用方处理）
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .post(&url)
        .json(&request_body)
        .send()
        .map_err(|e| format!("Cache API request failed: {}", e))?;

    let status = resp.status();
    let resp_json: Value = resp
        .json()
        .map_err(|e| format!("Failed to parse cache API response: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "Cache API returned {}: {}",
            status,
            resp_json.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).unwrap_or("unknown error")
        ));
    }

    resp_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Cache API response missing 'name' field".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compute_hash_deterministic() {
        let body1 = json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "system_instruction": "Be helpful"
        });
        let body2 = json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "system_instruction": "Be helpful"
        });
        assert_eq!(
            GeminiCacheStore::compute_hash(&body1),
            GeminiCacheStore::compute_hash(&body2)
        );
    }

    #[test]
    fn test_compute_hash_different_content() {
        let body1 = json!({
            "contents": [{"parts": [{"text": "Hello"}]}]
        });
        let body2 = json!({
            "contents": [{"parts": [{"text": "World"}]}]
        });
        assert_ne!(
            GeminiCacheStore::compute_hash(&body1),
            GeminiCacheStore::compute_hash(&body2)
        );
    }

    #[test]
    fn test_cache_store() {
        let store = GeminiCacheStore::new();
        assert!(store.get_cache("nonexistent").is_none());
        store.set_cache("hash123".into(), "cachedContents/abc".into());
        assert_eq!(
            store.get_cache("hash123"),
            Some("cachedContents/abc".into())
        );
    }
}
