use anyhow::{Result, ensure};
use firemage_egress_policy::{UpstreamProxy, ValueSource};
use firemage_request_signing::SecretResolver;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector,
    rustls::{self, pki_types::ServerName},
};

pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
pub(crate) type Stream = Box<dyn Io>;

pub(crate) async fn connect(
    target: SocketAddr,
    upstream: Option<&UpstreamProxy>,
    secrets: &dyn SecretResolver,
) -> Result<Stream> {
    let Some(proxy) = upstream else {
        return Ok(Box::new(TcpStream::connect(target).await?));
    };
    let url = url::Url::parse(&proxy.url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("proxy host missing"))?
        .trim_matches(['[', ']']);
    let port = url.port_or_known_default().unwrap_or(1080);
    let mut stream: Stream = Box::new(TcpStream::connect((host, port)).await?);
    if url.scheme() == "https" {
        stream = tls(stream, host, proxy.ca_pem.as_deref()).await?;
    }
    let credentials = match (&proxy.username, &proxy.password) {
        (Some(user), Some(password)) => {
            Some((value(user, secrets).await?, value(password, secrets).await?))
        }
        _ => None,
    };
    if url.scheme() == "socks5" {
        crate::upstream::socks5(&mut stream, target, credentials).await?;
    } else {
        crate::upstream::http_connect(&mut stream, target, credentials).await?;
    }
    Ok(stream)
}

async fn value(source: &ValueSource, secrets: &dyn SecretResolver) -> Result<String> {
    match source {
        ValueSource::Literal(value) => Ok(value.clone()),
        ValueSource::Secret { secret, prefix } => {
            Ok(format!("{}{}", prefix, secrets.resolve(secret).await?))
        }
    }
}

pub fn validate_ca_pem(pem: &str) -> Result<()> {
    roots(Some(pem)).map(|_| ())
}

fn roots(pem: Option<&str>) -> Result<rustls::RootCertStore> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(pem) = pem {
        let certificates =
            rustls_pemfile::certs(&mut pem.as_bytes()).collect::<std::io::Result<Vec<_>>>()?;
        ensure!(!certificates.is_empty(), "CA bundle has no certificates");
        for certificate in certificates {
            roots.add(certificate)?;
        }
    }
    Ok(roots)
}

pub(crate) async fn tls(stream: Stream, host: &str, pem: Option<&str>) -> Result<Stream> {
    let roots = roots(pem)?;
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Box::new(
        TlsConnector::from(Arc::new(config))
            .connect(ServerName::try_from(host.to_owned())?, stream)
            .await?,
    ))
}
