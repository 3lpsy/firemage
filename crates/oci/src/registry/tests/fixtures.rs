use axum::{Router, serve::Listener};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, rustls, server::TlsStream};

pub(super) struct Server {
    pub origin: String,
    pub pem: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub async fn start(router: Router) -> Self {
        let certified = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![certified.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der())
                .into(),
        )
        .unwrap();
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("https://{}", tcp.local_addr().unwrap());
        let listener = SecureListener {
            tcp,
            acceptor: TlsAcceptor::from(Arc::new(config)),
        };
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            origin,
            pem: certified.cert.pem(),
            task,
        }
    }
    pub fn image(&self, digest: &str) -> String {
        format!(
            "{}/test/image@{digest}",
            self.origin.trim_start_matches("https://")
        )
    }
    pub fn registry(
        &self,
        digest: &str,
        credentials: crate::RegistryCredentials,
    ) -> super::super::Registry {
        super::super::Registry::new(
            &self.image(digest),
            &crate::RegistryOptions {
                credentials,
                ca_pem: Some(self.pem.clone()),
                ..Default::default()
            },
        )
        .unwrap()
    }
}
struct SecureListener {
    tcp: TcpListener,
    acceptor: TlsAcceptor,
}
impl Listener for SecureListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (tcp, address) = self.tcp.accept().await.unwrap();
            if let Ok(tls) = self.acceptor.accept(tcp).await {
                return (tls, address);
            }
        }
    }
    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

pub(super) fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}
