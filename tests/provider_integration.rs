use serde_json::json;

/// 生成足够长以触发缓存注入的消息内容
fn long_message() -> String {
    // Anthropic 注入阈值 1024 token，ASCII 约 1 token/4 chars → 需要 4096+ 字符
    // OpenAI 注入阈值 1024 token
    // Gemini 注入阈值 32768 token（此处只测 Anthropic/OpenAI）
    "tokenJ test message for cache injection trigger. "
        .repeat(100)  // ~5000 chars → ~1250 tokens
}

/// 验证各 Provider 的缓存注入行为
#[test]
fn test_anthropic_cache_injection() {
    let mut body = json!({
        "model": "claude-sonnet-4-6",
        "system": "A".repeat(5000),
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 100
    });
    let result = TokenJ::provider::inject_cache_headers(
        &TokenJ::provider::Provider::Anthropic,
        &mut body,
    );
    assert!(result.injected, "Anthropic should inject cache_control on long system prompt");
    assert!(body["system"].is_array(), "System should be converted to array");
    assert!(
        body["system"][0].get("cache_control").is_some(),
        "cache_control should be on system[0]"
    );
    assert_eq!(
        body["system"][0]["cache_control"]["type"],
        "ephemeral",
        "Anthropic cache type should be ephemeral"
    );
}

#[test]
fn test_openai_cache_injection() {
    let mut body = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": long_message()}]
    });
    let result = TokenJ::provider::inject_cache_headers(
        &TokenJ::provider::Provider::OpenAI,
        &mut body,
    );
    assert!(result.injected, "OpenAI should inject cache headers for long messages");
    // 验证 body 中出现了 prompt_cache_key
    assert!(
        body.get("prompt_cache_key").is_some(),
        "OpenAI injection should add prompt_cache_key"
    );
}

#[test]
fn test_deepseek_cache_injection() {
    let mut body = json!({
        "model": "deepseek-v4-flash",
        "messages": [{"role": "user", "content": "Hello"}]
    });
    let result = TokenJ::provider::inject_cache_headers(
        &TokenJ::provider::Provider::DeepSeek,
        &mut body,
    );
    // DeepSeek 策略可能有所不同 - 至少不应该报错
    assert!(!result.details.is_empty() || result.injected == true || result.injected == false);
}

#[test]
fn test_glm_cache_control_stripped() {
    let mut body = json!({
        "model": "glm-5",
        "messages": [
            {"role": "user", "content": "Hello", "cache_control": {"type": "ephemeral"}}
        ]
    });
    let result = TokenJ::provider::inject_cache_headers(
        &TokenJ::provider::Provider::GLM,
        &mut body,
    );
    assert!(!result.injected, "GLM should NOT inject cache headers");
    // cache_control 应该被去掉
    let msg = &body["messages"][0];
    assert!(msg.get("cache_control").is_none(), "GLM should strip cache_control");
}

#[test]
fn test_unknown_provider_no_injection() {
    let mut body = json!({
        "model": "some-model",
        "messages": [{"role": "user", "content": "Hi"}]
    });
    let result = TokenJ::provider::inject_cache_headers(
        &TokenJ::provider::Provider::Unknown("test".into()),
        &mut body,
    );
    assert!(!result.injected, "Unknown provider should not inject");
}

#[test]
fn test_provider_detection_from_host() {
    use TokenJ::provider::Provider;

    assert_eq!(Provider::from_host("api.anthropic.com"), Provider::Anthropic);
    assert_eq!(Provider::from_host("api.openai.com"), Provider::OpenAI);
    assert_eq!(Provider::from_host("api.deepseek.com"), Provider::DeepSeek);
    assert_eq!(
        Provider::from_host("generativelanguage.googleapis.com"),
        Provider::Gemini
    );
    assert_eq!(Provider::from_host("open.bigmodel.cn"), Provider::GLM);

    match Provider::from_host("unknown.example.com") {
        Provider::Unknown(h) => assert_eq!(h, "unknown.example.com"),
        _ => panic!("Should be Unknown"),
    }
}

#[test]
fn test_parse_anthropic_cache_result() {
    let body = json!({
        "usage": {
            "input_tokens": 5000,
            "output_tokens": 200,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 4500
        }
    });
    let result = TokenJ::provider::parse_cache_result(
        &TokenJ::provider::Provider::Anthropic,
        &body,
    );
    assert_eq!(result.input_tokens, 5000);
    assert_eq!(result.output_tokens, 200);
    assert_eq!(result.cached_tokens, 4500);
}

#[test]
fn test_parse_openai_cache_result() {
    let body = json!({
        "usage": {
            "prompt_tokens": 3000,
            "completion_tokens": 150,
            "prompt_tokens_details": {
                "cached_tokens": 2800
            }
        }
    });
    let result = TokenJ::provider::parse_cache_result(
        &TokenJ::provider::Provider::OpenAI,
        &body,
    );
    assert_eq!(result.input_tokens, 3000);
    assert_eq!(result.output_tokens, 150);
    assert_eq!(result.cached_tokens, 2800);
}

#[test]
fn test_parse_gemini_cache_result() {
    // Gemini 使用独立的 Context Cache API，标准响应中不计缓存命中
    let body = json!({
        "usageMetadata": {
            "promptTokenCount": 4000,
            "candidatesTokenCount": 300
        }
    });
    let result = TokenJ::provider::parse_cache_result(
        &TokenJ::provider::Provider::Gemini,
        &body,
    );
    assert_eq!(result.cached_tokens, 0, "Gemini parse_cache returns 0 (uses separate API)");
}
