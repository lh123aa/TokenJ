use serde_json::Value;

pub mod anthropic;
pub mod deepseek;
pub mod gemini;
pub mod gemini_cache;
pub mod openai;

/// 字符类型感知的 Token 估算
///
/// - ASCII (英文/数字/符号): ~1 token / 4 chars
/// - 非 ASCII (中文/日文/韩文): ~1 token / 1.8 chars
/// - 空格和标点: 按比例计入
///
/// 这不是精确计数（真实 token 数取决于具体模型的 tokenizer），
/// 但比简单的 `len/4` 在中英文混合场景下准确 2-3 倍。
pub fn estimate_tokens(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    let (ascii_chars, non_ascii_chars) = text.chars().fold((0u64, 0u64), |(a, na), c| {
        if c.is_ascii() {
            (a + 1, na)
        } else {
            (a, na + 1)
        }
    });
    // ASCII: ~1 token per 4 chars;  CJK: ~1 token per 1.8 chars
    let ascii_tokens = (ascii_chars as f64 / 4.0).ceil() as u64;
    let non_ascii_tokens = (non_ascii_chars as f64 / 1.8).ceil() as u64;
    let total = ascii_tokens + non_ascii_tokens;
    if total < 1 {
        1
    } else {
        total
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    DeepSeek,
    Gemini,
    GLM,
    Unknown(String),
}

impl Provider {
    pub fn from_host(host: &str) -> Self {
        let host = host.to_lowercase();
        match host.as_str() {
            h if h.contains("anthropic.com") => Provider::Anthropic,
            h if h.contains("api.openai.com") => Provider::OpenAI,
            h if h.contains("openai.com") => Provider::OpenAI,
            h if h.contains("deepseek.com") => Provider::DeepSeek,
            h if h.contains("googleapis.com") => Provider::Gemini,
            h if h.contains("bigmodel.cn") => Provider::GLM,
            h => Provider::Unknown(h.to_string()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::DeepSeek => "deepseek",
            Provider::Gemini => "gemini",
            Provider::GLM => "glm",
            Provider::Unknown(_) => "unknown",
        }
    }
}

pub struct CacheInjection {
    pub injected: bool,
    pub details: Vec<String>,
}

pub fn inject_cache_headers(provider: &Provider, body: &mut Value) -> CacheInjection {
    match provider {
        Provider::Anthropic => anthropic::inject(body),
        Provider::OpenAI => openai::inject(body),
        Provider::DeepSeek => deepseek::inject(body),
        Provider::Gemini => gemini::inject(body),
        Provider::GLM => strip_cache_control(body),
        Provider::Unknown(_) => CacheInjection {
            injected: false,
            details: vec!["Unknown provider, no injection".into()],
        },
    }
}

#[derive(Debug, Clone, Default)]
pub struct CacheResult {
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub fn parse_cache_result(provider: &Provider, body: &Value) -> CacheResult {
    match provider {
        Provider::Anthropic => anthropic::parse_cache(body),
        Provider::OpenAI => openai::parse_cache(body),
        Provider::DeepSeek => deepseek::parse_cache(body),
        Provider::Gemini => gemini::parse_cache(body),
        _ => CacheResult {
            cached_tokens: 0,
            cache_write_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
        },
    }
}

fn strip_cache_control(body: &mut Value) -> CacheInjection {
    let stripped = remove_cache_control(body);
    CacheInjection {
        injected: false,
        details: if stripped > 0 {
            vec![format!("Stripped {} cache_control fields (not supported)", stripped)]
        } else {
            vec!["Provider does not support caching".into()]
        },
    }
}

fn remove_cache_control(value: &mut Value) -> u32 {
    let mut count = 0;
    match value {
        Value::Object(map) => {
            if map.remove("cache_control").is_some() {
                count += 1;
            }
            for val in map.values_mut() {
                count += remove_cache_control(val);
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                count += remove_cache_control(val);
            }
        }
        _ => {}
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_host_anthropic() {
        assert_eq!(Provider::from_host("api.anthropic.com"), Provider::Anthropic);
        assert_eq!(Provider::from_host("ANTHROPIC.COM"), Provider::Anthropic);
    }

    #[test]
    fn test_provider_from_host_openai() {
        assert_eq!(Provider::from_host("api.openai.com"), Provider::OpenAI);
        assert_eq!(Provider::from_host("api.openai.com/v1"), Provider::OpenAI);
    }

    #[test]
    fn test_provider_from_host_deepseek() {
        assert_eq!(Provider::from_host("api.deepseek.com"), Provider::DeepSeek);
    }

    #[test]
    fn test_provider_from_host_gemini() {
        assert_eq!(Provider::from_host("generativelanguage.googleapis.com"), Provider::Gemini);
    }

    #[test]
    fn test_provider_from_host_glm() {
        assert_eq!(Provider::from_host("open.bigmodel.cn"), Provider::GLM);
    }

    #[test]
    fn test_provider_from_host_unknown() {
        match Provider::from_host("example.com") {
            Provider::Unknown(h) => assert_eq!(h, "example.com"),
            _ => panic!("Should be Unknown"),
        }
    }

    #[test]
    fn test_provider_names() {
        assert_eq!(Provider::Anthropic.name(), "anthropic");
        assert_eq!(Provider::OpenAI.name(), "openai");
        assert_eq!(Provider::DeepSeek.name(), "deepseek");
        assert_eq!(Provider::Gemini.name(), "gemini");
        assert_eq!(Provider::GLM.name(), "glm");
        assert_eq!(Provider::Unknown("x".into()).name(), "unknown");
    }

    #[test]
    fn test_inject_cache_unknown_provider() {
        let mut body = serde_json::json!({"model": "test"});
        let result = inject_cache_headers(&Provider::Unknown("x".into()), &mut body);
        assert!(!result.injected);
    }

    #[test]
    fn test_strip_cache_control_from_glm() {
        let mut body = serde_json::json!({
            "model": "glm-5",
            "messages": [{"role": "user", "content": "hi", "cache_control": {"type": "ephemeral"}}]
        });
        let result = inject_cache_headers(&Provider::GLM, &mut body);
        assert!(!result.injected);
        assert!(result.details[0].contains("Stripped"));
        // Verify cache_control was removed
        let msg = &body["messages"][0];
        assert!(msg.get("cache_control").is_none());
    }

    #[test]
    fn test_parse_cache_result_unknown_provider() {
        let body = serde_json::json!({"usage": {"input_tokens": 100}});
        let result = parse_cache_result(&Provider::Unknown("x".into()), &body);
        assert_eq!(result.input_tokens, 0);
    }
}
