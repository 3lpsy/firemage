use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
};
use firemage_wire::{Kernel, KernelAlias, KernelImport};

pub async fn list(_identity: Identity, State(app): State<App>) -> Result<Json<Vec<Kernel>>> {
    let _guard = app.runtime.lock("kernels").await;
    Ok(Json(app.runtime.kernels().await?))
}
pub async fn upload(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<Json<Kernel>> {
    identity.ensure_admin()?;
    firemage_wire::ensure_kernel_name(&name)?;
    let _guard = app.runtime.lock("kernels").await;
    let catalog = firemage_kernels::Catalog::open(&app.runtime.config.kernel_dir())?;
    let filename = name.clone();
    tokio::task::spawn_blocking(move || catalog.upload(&filename, &body))
        .await
        .map_err(anyhow::Error::from)??;
    firemage_queries::set_kernel_alias(&app.runtime.db, &name, None).await?;
    app.record(&identity, "kernel.upload", &name).await?;
    Ok(Json(app.runtime.kernel(&name).await?))
}
pub async fn import(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<KernelImport>,
) -> Result<Json<Kernel>> {
    identity.ensure_admin()?;
    let catalog = firemage_kernels::Catalog::open(&app.runtime.config.kernel_dir())?;
    let file = firemage_kernels::fetch(&catalog, &input).await?;
    let _guard = app.runtime.lock("kernels").await;
    catalog.publish(&input.name, file)?;
    firemage_queries::set_kernel_alias(&app.runtime.db, &input.name, None).await?;
    app.record(&identity, "kernel.import", &input.name).await?;
    Ok(Json(app.runtime.kernel(&input.name).await?))
}
pub async fn alias(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
    Json(input): Json<KernelAlias>,
) -> Result<Json<Kernel>> {
    identity.ensure_admin()?;
    let kernel = app.runtime.alias_kernel(&name, &input).await?;
    app.record(&identity, "kernel.alias", &name).await?;
    Ok(Json(kernel))
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    app.runtime.delete_kernel(&name).await?;
    app.record(&identity, "kernel.delete", &name).await?;
    Ok(StatusCode::NO_CONTENT)
}
