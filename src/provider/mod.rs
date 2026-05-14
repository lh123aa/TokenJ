use serde_json::Value;

pub mod anthropic;
pub mod deepseek;
pub mod gemini;
pub mod openai;

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
        match host {
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
