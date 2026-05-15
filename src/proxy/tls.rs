pub fn is_llm_domain(host: &str) -> bool {
    let host = host.to_lowercase();
    let llm_domains = [
        "api.openai.com",
        "api.anthropic.com",
        "api.deepseek.com",
        "generativelanguage.googleapis.com",
        "open.bigmodel.cn",
    ];
    llm_domains.iter().any(|d| host.contains(d))
}

pub fn parse_host(authority: &str) -> (String, u16) {
    if let Some(colon_pos) = authority.rfind(':') {
        let host = &authority[..colon_pos];
        let port: u16 = authority[colon_pos + 1..].parse().unwrap_or(443);
        (host.to_string(), port)
    } else {
        (authority.to_string(), 443)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_llm_domain_anthropic() {
        assert!(is_llm_domain("api.anthropic.com"));
        assert!(is_llm_domain("api.anthropic.com:443"));
        assert!(is_llm_domain("api.ANTHROPIC.COM"));
    }

    #[test]
    fn test_is_llm_domain_openai() {
        assert!(is_llm_domain("api.openai.com"));
        assert!(is_llm_domain("api.openai.com/v1"));
    }

    #[test]
    fn test_is_llm_domain_deepseek() {
        assert!(is_llm_domain("api.deepseek.com"));
    }

    #[test]
    fn test_is_llm_domain_gemini() {
        assert!(is_llm_domain("generativelanguage.googleapis.com"));
    }

    #[test]
    fn test_is_llm_domain_glm() {
        assert!(is_llm_domain("open.bigmodel.cn"));
    }

    #[test]
    fn test_is_not_llm_domain() {
        assert!(!is_llm_domain("example.com"));
        assert!(!is_llm_domain("google.com"));
        assert!(!is_llm_domain(""));
    }

    #[test]
    fn test_parse_host_with_port() {
        let (host, port) = parse_host("api.anthropic.com:443");
        assert_eq!(host, "api.anthropic.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_host_without_port() {
        let (host, port) = parse_host("api.anthropic.com");
        assert_eq!(host, "api.anthropic.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_host_custom_port() {
        let (host, port) = parse_host("api.openai.com:8080");
        assert_eq!(host, "api.openai.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_host_ipv6_like() {
        let (host, port) = parse_host("[::1]:443");
        assert_eq!(host, "[::1]");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_host_invalid_port_fallback() {
        let (host, port) = parse_host("example.com:abc");
        assert_eq!(host, "example.com");
        // port parsing fails, falls back to 443
        assert_eq!(port, 443);
    }
}
