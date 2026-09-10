use anyhow::{Context, Result, ensure};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use std::{fs, io::Write, path::Path, sync::Arc};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{self, pki_types::PrivatePkcs8KeyDer},
};

pub(crate) struct Authority {
    cert: Certificate,
    key: KeyPair,
    pem: String,
    fingerprint: String,
}

impl Authority {
    pub fn load(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
        let key_path = directory.join("egress-ca.key");
        let cert_path = directory.join("egress-ca.pem");
        let (params, key, existing) = if key_path.exists() || cert_path.exists() {
            ensure!(
                fs::symlink_metadata(&key_path)?.file_type().is_file()
                    && fs::symlink_metadata(&cert_path)?.file_type().is_file(),
                "CA files must be regular files"
            );
            let pem = fs::read_to_string(&cert_path)?;
            (
                CertificateParams::from_ca_cert_pem(&pem)?,
                KeyPair::from_pem(&fs::read_to_string(&key_path)?)?,
                Some(pem),
            )
        } else {
            let mut params = CertificateParams::default();
            params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
            params
                .distinguished_name
                .push(DnType::CommonName, "Firemage guest egress CA");
            (params, KeyPair::generate()?, None)
        };
        let cert = params.self_signed(&key)?;
        let pem = existing.unwrap_or_else(|| cert.pem());
        if !key_path.exists() {
            write_private(&key_path, key.serialize_pem().as_bytes())?;
            write_private(&cert_path, pem.as_bytes())?;
        }
        let certificates =
            rustls_pemfile::certs(&mut pem.as_bytes()).collect::<std::io::Result<Vec<_>>>()?;
        ensure!(
            certificates.len() == 1,
            "egress CA must contain exactly one certificate"
        );
        let (_, parsed) = x509_parser::parse_x509_certificate(certificates[0].as_ref())
            .map_err(|_| anyhow::anyhow!("invalid egress CA certificate"))?;
        ensure!(
            parsed.public_key().raw == key.public_key_der(),
            "egress CA certificate does not match its private key"
        );
        use sha2::Digest;
        let fingerprint = format!(
            "SHA256:{}",
            hex::encode(sha2::Sha256::digest(certificates[0].as_ref()))
        );
        Ok(Self {
            cert,
            key,
            pem,
            fingerprint,
        })
    }

    pub fn fingerprint(&self) -> String {
        self.fingerprint.clone()
    }

    pub fn pem(&self) -> String {
        self.pem.clone()
    }

    pub fn acceptor(&self, host: &str) -> Result<TlsAcceptor> {
        let key = KeyPair::generate()?;
        let cert = CertificateParams::new(vec![host.to_owned()])?
            .signed_by(&key, &self.cert, &self.key)?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.der().clone()],
                PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
            )?;
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(TlsAcceptor::from(Arc::new(config)))
    }
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).context("create private egress CA")?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}
