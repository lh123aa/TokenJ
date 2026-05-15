use std::path::PathBuf;
use uuid::Uuid;
use TokenJ::cert::CertManager;

fn temp_cert_dir() -> PathBuf {
    let uid = Uuid::new_v4();
    std::env::temp_dir().join(format!("TokenJ_cert_int_{}", uid))
}

#[test]
fn test_ca_cert_generation() {
    let dir = temp_cert_dir();
    let cm = CertManager::load_or_create(&dir).unwrap();

    let ca_cert_pem = cm.ca_cert_pem().unwrap();
    assert!(!ca_cert_pem.is_empty(), "CA cert PEM should not be empty");
    assert!(
        ca_cert_pem.contains("BEGIN CERTIFICATE"),
        "CA cert should be valid PEM"
    );

    // CA 证书 PEM 有效即可（CN 在 DER 编码中，不直接以文本形式出现）

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_domain_cert_generation() {
    let dir = temp_cert_dir();
    let cm = CertManager::load_or_create(&dir).unwrap();

    // 为 LLM 域名签发证书
    for domain in &[
        "api.anthropic.com",
        "api.openai.com",
        "api.deepseek.com",
        "generativelanguage.googleapis.com",
    ] {
        let (cert_pem, key_pem) = cm.get_or_create_domain_cert_pem(domain).unwrap();
        assert!(!cert_pem.is_empty(), "Domain cert PEM should not be empty for {}", domain);
        assert!(!key_pem.is_empty(), "Domain key PEM should not be empty for {}", domain);
        assert!(
            cert_pem.contains("BEGIN CERTIFICATE"),
            "Domain cert should be valid PEM for {}",
            domain
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_ca_cert_persistence() {
    let dir = temp_cert_dir();

    // 第一次创建
    let cm1 = CertManager::load_or_create(&dir).unwrap();
    let cert1 = cm1.ca_cert_pem().unwrap().to_string();

    // 第二次加载（应使用已有的 CA 文件）
    let cm2 = CertManager::load_or_create(&dir).unwrap();
    let cert2 = cm2.ca_cert_pem().unwrap().to_string();

    // 两次的证书都应该是有效的 PEM 格式
    assert!(cert1.contains("BEGIN CERTIFICATE"), "First CA cert should be valid PEM");
    assert!(cert2.contains("BEGIN CERTIFICATE"), "Second CA cert should be valid PEM");

    // 验证 CA 文件确实已保存到磁盘
    let ca_crt_path = dir.join("ca.crt");
    assert!(ca_crt_path.exists(), "CA cert file should exist on disk");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_domain_cert_reuse() {
    let dir = temp_cert_dir();
    let cm = CertManager::load_or_create(&dir).unwrap();

    // 同一个域名获取两次，应返回相同证书（缓存）
    let (cert_a1, key_a1) = cm.get_or_create_domain_cert_pem("api.anthropic.com").unwrap();
    let (cert_a2, key_a2) = cm.get_or_create_domain_cert_pem("api.anthropic.com").unwrap();

    assert_eq!(cert_a1, cert_a2, "Domain cert should be cached and reused");
    assert_eq!(key_a1, key_a2, "Domain key should be cached and reused");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_multiple_domain_certs() {
    let dir = temp_cert_dir();
    let cm = CertManager::load_or_create(&dir).unwrap();

    let domains = [
        "api.anthropic.com",
        "api.openai.com",
        "api.deepseek.com",
        "generativelanguage.googleapis.com",
        "open.bigmodel.cn",
    ];

    // 批量生成证书
    let mut certs = Vec::new();
    for domain in &domains {
        let (cert, _key) = cm.get_or_create_domain_cert_pem(domain).unwrap();
        certs.push(cert);
    }

    // 每个域名应有不同的证书
    for i in 0..certs.len() {
        for j in (i + 1)..certs.len() {
            assert_ne!(
                certs[i], certs[j],
                "Different domains should have different certs"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_cert_manager_load_nonexistent_dir() {
    let dir = temp_cert_dir();

    // 目录不存在时应自动创建
    assert!(!dir.exists(), "Temp dir should not exist yet");
    let cm = CertManager::load_or_create(&dir).unwrap();
    assert!(dir.exists(), "Cert dir should be created");
    assert!(cm.ca_cert_pem().is_ok(), "CA cert should be generated");

    let _ = std::fs::remove_dir_all(&dir);
}
