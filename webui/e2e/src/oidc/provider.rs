use anyhow::Result;
use axum::{
    Router,
    routing::{get, post},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub(super) struct ProviderState {
    pub issuer: String,
    pub callback: String,
    pub client_secret: Option<String>,
    pub last_callback: Arc<Mutex<Option<String>>>,
    pub attempts: Arc<Mutex<HashMap<String, Attempt>>>,
}
pub(super) struct Attempt {
    pub nonce: String,
    pub challenge: String,
    pub state: String,
    pub subject: Option<String>,
}
pub(crate) struct MockProvider {
    pub issuer: String,
    pub ca_cert: PathBuf,
    pub last_callback: Arc<Mutex<Option<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for MockProvider {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl MockProvider {
    pub async fn new(
        directory: &Path,
        public_url: &str,
        client_secret: Option<String>,
    ) -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (ca_cert, cert, key) = super::tls::certificates(directory)?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let issuer = format!("https://{}", listener.local_addr()?);
        let last_callback = Arc::new(Mutex::new(None));
        let state = ProviderState {
            last_callback: last_callback.clone(),
            client_secret,
            issuer: issuer.clone(),
            callback: format!("{public_url}/v1/browser/oidc/callback"),
            attempts: Default::default(),
        };
        let app = Router::new()
            .route(
                "/.well-known/openid-configuration",
                get(super::routes::discovery),
            )
            .route("/keys", get(super::routes::keys))
            .route(
                "/authorize",
                get(super::routes::authorize).post(super::routes::login),
            )
            .route("/token", post(super::routes::token))
            .with_state(state);
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
        let task = tokio::spawn(async move {
            if let Err(error) = axum_server::from_tcp_rustls(listener, tls)
                .serve(app.into_make_service())
                .await
            {
                eprintln!("OIDC fixture server: {error}");
            }
        });
        Ok(Self {
            issuer,
            ca_cert,
            last_callback,
            task,
        })
    }
}
