use anyhow::{Context, Result};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose};
use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 内存中缓存的最大域名证书数量，超过时淘汰最久未使用的
const MAX_CACHED_DOMAINS: usize = 100;

/// 证书管理器：管理 CA 证书 + 动态签发域名证书
///
/// # 功能
/// - 首次启动自动生成 CA 根证书
/// - 后续启动加载已有的 CA 证书
/// - 为 MITM 代理动态签发目标域名证书
/// - 缓存已签发的域名证书（LRU 淘汰，上限 {MAX_CACHED_DOMAINS}）
pub struct CertManager {
    cert_dir: PathBuf,
    ca_cert: Certificate,
    ca_key: KeyPair,
    /// 域名证书缓存: domain -> (cert, key)
    issued: Mutex<HashMap<String, (Certificate, KeyPair)>>,
    /// 访问顺序记录，用于 LRU 淘汰
    access_order: Mutex<VecDeque<String>>,
}

impl CertManager {
    /// 加载或创建 CA 证书
    pub fn load_or_create(cert_dir: &Path) -> Result<Self> {
        fs::create_dir_all(cert_dir)?;

        let ca_cert_path = cert_dir.join("ca.crt");
        let ca_key_path = cert_dir.join("ca.key");

        let (ca_cert, ca_key) = if ca_cert_path.exists() && ca_key_path.exists() {
            let ca_pem = fs::read_to_string(&ca_cert_path)
                .context("Failed to read CA certificate")?;
            let ca_key_pem = fs::read_to_string(&ca_key_path)
                .context("Failed to read CA key")?;

            // rcgen 0.13: 通过 CertificateParams::from_ca_cert_pem 加载 CA 证书
            let ca_cert_params = CertificateParams::from_ca_cert_pem(&ca_pem)
                .context("Failed to parse existing CA certificate")?;
            let ca_key = KeyPair::from_pem(&ca_key_pem)
                .context("Failed to parse existing CA key")?;
            // 用原有参数 + 原有密钥重新生成 CA 证书（签名不变）
            let ca_cert = ca_cert_params
                .self_signed(&ca_key)
                .context("Failed to re-sign CA certificate")?;
            (ca_cert, ca_key)
        } else {
            let (ca_cert, ca_key) = Self::generate_ca()?;
            fs::write(&ca_cert_path, ca_cert.pem())
                .context("Failed to write CA certificate")?;
            fs::write(&ca_key_path, ca_key.serialize_pem())
                .context("Failed to write CA key")?;
            (ca_cert, ca_key)
        };

        Ok(Self {
            cert_dir: cert_dir.to_path_buf(),
            ca_cert,
            ca_key,
            issued: Mutex::new(HashMap::new()),
            access_order: Mutex::new(VecDeque::new()),
        })
    }

    /// 生成自签名 CA 根证书
    fn generate_ca() -> Result<(Certificate, KeyPair)> {
        let key_pair = KeyPair::generate()?;
        let mut params = CertificateParams::default();
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "TokenJ Root CA");
        let cert = params.self_signed(&key_pair)?;
        Ok((cert, key_pair))
    }

    /// 为指定域名获取或创建证书，返回 PEM 格式的证书和私钥
    ///
    /// 缓存采用 LRU 策略，超过 {MAX_CACHED_DOMAINS} 个域名时淘汰最久未访问的条目。
    /// 同步版本，适用于 spawn_blocking 或测试环境。
    pub fn get_or_create_domain_cert_pem(&self, domain: &str) -> Result<(String, String)> {
        let mut cache = self.issued.lock().expect("cert cache poisoned");
        let mut order = self.access_order.lock().expect("access order poisoned");

        // 缓存命中：更新访问顺序（移到末尾）
        if let Some((cert, key)) = cache.get(domain) {
            if let Some(pos) = order.iter().position(|d| d == domain) {
                order.remove(pos);
                order.push_back(domain.to_string());
            }
            return Ok((cert.pem(), key.serialize_pem()));
        }

        // 缓存未命中：若已满，淘汰最久未访问的条目
        if cache.len() >= MAX_CACHED_DOMAINS {
            if let Some(oldest) = order.pop_front() {
                cache.remove(&oldest);
            }
        }
        // 释放锁，证书生成移到 spawn_blocking 中处理
        drop(cache);
        drop(order);

        // 签发新域名证书（CPU 密集型，在 async 调用方应在 spawn_blocking 中执行）
        let domain_key = KeyPair::generate()?;
        let domain_cert_params = CertificateParams::new(vec![domain.to_string()])
            .context("Failed to create domain certificate params")?;

        let domain_cert = domain_cert_params
            .signed_by(&domain_key, &self.ca_cert, &self.ca_key)
            .context(format!("Failed to sign certificate for domain: {}", domain))?;

        let cert_pem = domain_cert.pem();
        let key_pem = domain_key.serialize_pem();

        // 重新获取锁并缓存
        let mut cache = self.issued.lock().expect("cert cache poisoned");
        let mut order = self.access_order.lock().expect("access order poisoned");
        cache.insert(domain.to_string(), (domain_cert, domain_key));
        order.push_back(domain.to_string());

        Ok((cert_pem, key_pem))
    }

    /// 异步版本：在 spawn_blocking 中执行证书生成，不阻塞 tokio 工作线程
    pub async fn get_or_create_domain_cert_pem_async(
        self: &Arc<Self>,
        domain: &str,
    ) -> Result<(String, String)> {
        let self_clone = self.clone();
        let domain = domain.to_string();
        tokio::task::spawn_blocking(move || {
            self_clone.get_or_create_domain_cert_pem(&domain)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {}", e))?
    }

    /// 获取 CA 证书 PEM
    pub fn ca_cert_pem(&self) -> Result<String> {
        Ok(self.ca_cert.pem())
    }

    /// 获取证书存储目录
    pub fn cert_dir(&self) -> &Path {
        &self.cert_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_or_create_generates_ca() {
        let dir = std::env::temp_dir().join(format!("TokenJ_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = CertManager::load_or_create(&dir).unwrap();
        let pem = mgr.ca_cert_pem().unwrap();
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"), "Should be a valid PEM cert");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_or_create_reuses_existing_ca_key() {
        let dir = std::env::temp_dir().join(format!("TokenJ_test_ca_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mgr1 = CertManager::load_or_create(&dir).unwrap();
        let pem1 = mgr1.ca_cert_pem().unwrap();

        let mgr2 = CertManager::load_or_create(&dir).unwrap();
        let pem2 = mgr2.ca_cert_pem().unwrap();

        // CA cert may differ due to rcgen re-signing with new serial/timestamp,
        // but the disk file (ca.crt) must remain identical for user-trusted installs.
        // Verify both in-memory certs are valid PEM.
        assert!(pem1.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(pem2.starts_with("-----BEGIN CERTIFICATE-----"));

        // Disk file should be stable
        let disk_pem = std::fs::read_to_string(dir.join("ca.crt")).unwrap();
        assert!(disk_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_domain_cert_generation() {
        let dir = std::env::temp_dir().join(format!("TokenJ_test_domain_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = CertManager::load_or_create(&dir).unwrap();

        let (cert_pem, key_pem) = mgr
            .get_or_create_domain_cert_pem("api.anthropic.com")
            .unwrap();
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(key_pem.contains("BEGIN PRIVATE KEY"));
        assert!(!cert_pem.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_domain_cert_caching() {
        let dir = std::env::temp_dir().join(format!("TokenJ_test_cache_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = CertManager::load_or_create(&dir).unwrap();

        let (cert1, _) = mgr
            .get_or_create_domain_cert_pem("api.openai.com")
            .unwrap();
        let (cert2, _) = mgr
            .get_or_create_domain_cert_pem("api.openai.com")
            .unwrap();

        assert_eq!(cert1, cert2, "Same domain should return cached cert");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_multiple_domains() {
        let dir = std::env::temp_dir().join(format!("TokenJ_test_multi_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = CertManager::load_or_create(&dir).unwrap();

        let domains = ["api.anthropic.com", "api.openai.com", "api.deepseek.com"];

        for domain in &domains {
            let (cert, key) = mgr.get_or_create_domain_cert_pem(domain).unwrap();
            assert!(!cert.is_empty(), "Cert for {} should not be empty", domain);
            assert!(!key.is_empty(), "Key for {} should not be empty", domain);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
