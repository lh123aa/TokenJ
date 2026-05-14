use anyhow::Result;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, KeyUsagePurpose};
use std::fs;
use std::path::Path;

pub struct CertManager {
    cert_dir: std::path::PathBuf,
}

impl CertManager {
    pub fn load_or_create(cert_dir: &Path) -> Result<Self> {
        fs::create_dir_all(cert_dir)?;
        Ok(Self { cert_dir: cert_dir.to_path_buf() })
    }

    pub fn ca_cert_pem(&self) -> Result<String> {
        let path = self.cert_dir.join("ca.crt");
        if path.exists() {
            Ok(fs::read_to_string(path)?)
        } else {
            let (ca_pem, ca_key_pem) = Self::generate_ca()?;
            fs::write(&path, &ca_pem)?;
            fs::write(self.cert_dir.join("ca.key"), &ca_key_pem)?;
            Ok(ca_pem)
        }
    }

    fn generate_ca() -> Result<(String, String)> {
        let key_pair = KeyPair::generate()?;
        let mut params = CertificateParams::default();
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "tokenJ Root CA");
        let cert = params.self_signed(&key_pair)?;
        Ok((cert.pem(), key_pair.serialize_pem()))
    }

    pub fn cert_dir(&self) -> &Path {
        &self.cert_dir
    }
}
