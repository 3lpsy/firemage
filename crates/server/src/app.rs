use crate::{identity, login, management, oidc, resources, tokens, users};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct App {
    pub runtime: firemage_runtime::Runtime,
    pub management: Arc<firemage_config::ManagedConfig>,
    pub(crate) started: std::time::Instant,
    pub(crate) browser: crate::browser::BrowserState,
    pub(crate) login_budget: Arc<Mutex<(std::time::Instant, u32)>>,
    pub(crate) dummy_hash: Arc<String>,
}
impl App {
    pub fn new(runtime: firemage_runtime::Runtime) -> anyhow::Result<Self> {
        Ok(Self {
            management: Arc::new(firemage_config::ManagedConfig::new(
                None,
                runtime.config.clone(),
                runtime.config.clone(),
            )),
            started: std::time::Instant::now(),
            browser: Default::default(),
            runtime,
            login_budget: Arc::new(Mutex::new((std::time::Instant::now(), 0))),
            dummy_hash: Arc::new(firemage_auth::hash_password(
                "invalid-placeholder-password",
            )?),
        })
    }
}
pub fn router(app: App) -> Router {
    Router::new()
        .merge(crate::egress_catalog::routes())
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status":"ok"})) }),
        )
        .route("/v1/auth/login", post(login::login))
        .route("/v1/auth/refresh", post(tokens::refresh))
        .route("/v1/auth/logout", post(tokens::logout))
        .route(
            "/v1/auth/oidc",
            get(oidc::configuration).post(oidc::exchange),
        )
        .route(
            "/v1/assets",
            get(crate::file_assets::list)
                .post(crate::file_assets::upload)
                .layer(DefaultBodyLimit::max(
                    app.runtime.config.asset_max_bytes() as usize
                )),
        )
        .route("/v1/assets/limits", get(crate::file_assets::limits))
        .route(
            "/v1/assets/{id}",
            axum::routing::put(crate::file_assets::alias).delete(crate::file_assets::delete),
        )
        .route("/v1/assets/{id}/content", get(crate::file_assets::content))
        .route("/v1/kernels", get(crate::kernels::list))
        .route("/v1/kernels/import", post(crate::kernels::import))
        .route(
            "/v1/kernels/{name}",
            axum::routing::put(crate::kernels::alias).delete(crate::kernels::delete),
        )
        .route(
            "/v1/kernels/{name}/content",
            axum::routing::put(crate::kernels::upload).layer(DefaultBodyLimit::max(
                firemage_wire::KERNEL_MAX_BYTES as usize,
            )),
        )
        .route(
            "/v1/snapshots",
            get(crate::snapshots::list)
                .post(crate::snapshots::upload)
                .layer(DefaultBodyLimit::disable()),
        )
        .route("/v1/snapshots/limits", get(crate::snapshots::limits))
        .route(
            "/v1/snapshots/{id}",
            axum::routing::delete(crate::snapshots::delete),
        )
        .route(
            "/v1/snapshots/{id}/download",
            get(crate::snapshots::download),
        )
        .route("/v1/snapshots/{id}/trust", post(crate::snapshots::trust))
        .route("/v1/vms/{id}/snapshots", post(crate::snapshots::save))
        .route(
            "/v1/vms/{id}/snapshots/restore",
            post(crate::snapshots::restore),
        )
        .route("/v1/secrets", get(crate::secrets::list))
        .route(
            "/v1/secrets/{name}",
            axum::routing::put(crate::secrets::put).delete(crate::secrets::delete),
        )
        .route("/v1/me", get(identity::me))
        .route("/v1/host", get(management::host))
        .route("/v1/activity", get(management::activity))
        .route(
            "/v1/config",
            get(management::configuration).put(management::save),
        )
        .route("/v1/config/validate", post(management::validate))
        .route(
            "/v1/users/{id}",
            axum::routing::put(users::update).delete(users::delete),
        )
        .route("/v1/apitokens", get(tokens::list).post(tokens::create))
        .route("/v1/apitokens/{id}", axum::routing::delete(tokens::delete))
        .route("/v1/users", get(users::list).post(users::create))
        .route(
            "/v1/vms",
            get(resources::list_vms).post(resources::create_vm),
        )
        .route("/v1/vm-config/preview", post(crate::vm_config::preview))
        .route("/v1/vm-config/import", post(crate::vm_config::import))
        .route(
            "/v1/vms/{id}/config",
            get(crate::vm_config::export).put(crate::vm_config::import_existing),
        )
        .route(
            "/v1/vms/{id}/config/preview",
            post(crate::vm_config::preview_existing),
        )
        .route(
            "/v1/vms/{id}",
            get(resources::get_vm)
                .put(resources::update_vm)
                .delete(resources::delete_vm),
        )
        .route("/v1/vms/{id}/attachments", get(crate::attachments::list))
        .route("/v1/vms/{id}/duplicate", post(crate::duplicate::duplicate))
        .route("/v1/vms/{id}/actions", post(resources::action))
        .route("/v1/vms/{id}/firecracker", post(resources::raw))
        .route("/v1/vms/{id}/files", get(resources::output))
        .route("/v1/vms/{id}/directory", get(crate::guest_files::directory))
        .route(
            "/v1/vms/{id}/files/download",
            get(crate::guest_files::download),
        )
        .route("/v1/vms/{id}/logs", get(crate::logs::read))
        .route(
            "/v1/vms/{id}/terminal",
            get(crate::terminal::status)
                .post(crate::terminal::input)
                .layer(DefaultBodyLimit::max(32 * 1024)),
        )
        .route("/v1/vms/{id}/egress", get(crate::egress::status))
        .route("/v1/egress/ca", get(crate::egress::ca))
        .route(
            "/v1/networks",
            get(resources::list_networks).post(resources::create_network),
        )
        .route(
            "/v1/networks/{name}/suggestion",
            get(crate::addressing::suggestion),
        )
        .route(
            "/v1/networks/{name}",
            axum::routing::delete(resources::delete_network).put(resources::update_network),
        )
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(axum::middleware::map_response(crate::error::json_errors))
        .merge(crate::browser::routes())
        .fallback(crate::assets::serve)
        .layer(axum::middleware::map_response(
            crate::browser::security_headers,
        ))
        .with_state(app)
}
pub async fn serve(config: firemage_config::Server) -> anyhow::Result<()> {
    serve_managed(config.clone(), None, config).await
}
pub async fn serve_managed(
    mut config: firemage_config::Server,
    path: Option<std::path::PathBuf>,
    overrides: firemage_config::Server,
) -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    config.validate()?;
    let management = Arc::new(firemage_config::ManagedConfig::new(
        path,
        overrides,
        config.clone(),
    ));
    tokio::fs::create_dir_all(config.data_dir()).await?;
    config.data_dir = Some(tokio::fs::canonicalize(config.data_dir()).await?);
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(config.data_dir(), std::fs::Permissions::from_mode(0o700)).await?;
    use std::os::unix::fs::OpenOptionsExt;
    let server_lock = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(config.data_dir().join("server.lock"))?;
    server_lock
        .try_lock()
        .map_err(|_| anyhow::anyhow!("another Firemage server is using this data directory"))?;
    let db = firemage_queries::connect(&config.database()).await?;
    let runtime = firemage_runtime::Runtime::new(db, config.clone());
    runtime.migrate_egress_catalog().await?;
    runtime.recover_egress().await?;
    let mut state = App::new(runtime.clone())?;
    state.management = management;
    let app = router(state);
    tokio::spawn(async move {
        loop {
            if let Ok(rows) = firemage_queries::vms(&runtime.db, None).await {
                for row in rows {
                    let _guard = runtime.lock(&row.id).await;
                    if let Ok(current) =
                        firemage_queries::vm(&runtime.db, &row.owner_id, &row.id).await
                    {
                        let _ = runtime.refresh(current).await;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
    crate::ensure!(
        config.tls_cert.is_some() == config.tls_key.is_some(),
        "tls-cert and tls-key must be set together"
    );
    if let Some(path) = &config.unix_socket {
        let socket =
            crate::socket::bind(path, config.unix_socket_mode()?, config.unix_socket_gid).await?;
        let (listener, _socket_guard) = socket;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown())
            .await?;
    } else {
        let address = config
            .listen
            .as_deref()
            .unwrap_or("127.0.0.1:8080")
            .parse::<std::net::SocketAddr>()?;
        if let (Some(cert), Some(key)) = (config.tls_cert, config.tls_key) {
            let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
            let handle = axum_server::Handle::new();
            let stop = handle.clone();
            tokio::spawn(async move {
                shutdown().await;
                stop.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
            });
            axum_server::bind_rustls(address, tls)
                .handle(handle)
                .serve(app.into_make_service())
                .await?;
        } else {
            crate::ensure!(
                address.ip().is_loopback(),
                "public listeners require TLS; bind loopback behind a reverse proxy"
            );
            axum::serve(tokio::net::TcpListener::bind(address).await?, app)
                .with_graceful_shutdown(shutdown())
                .await?;
        }
    }
    Ok(())
}
async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("signal handler");
    tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
}
