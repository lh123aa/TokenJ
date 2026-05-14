#[allow(dead_code)]
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
