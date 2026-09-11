use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
};
use firemage_wire::{Kernel, KernelAlias, KernelImport};

pub async fn list(_identity: Identity, State(app): State<App>) -> Result<Json<Vec<Kernel>>> {
    Ok(Json(app.runtime.kernels().await?))
}
pub async fn upload(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
    Query(alias): Query<KernelAlias>,
    body: Bytes,
) -> Result<Json<Kernel>> {
    identity.ensure_admin()?;
    alias.validate()?;
    let kernel = app
        .runtime
        .upload_kernel(
            &name,
            alias.alias.as_deref().unwrap_or_default(),
            body.to_vec(),
        )
        .await?;
    app.record(&identity, "kernel.upload", &name).await?;
    Ok(Json(kernel))
}
pub async fn import(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<KernelImport>,
) -> Result<Json<Kernel>> {
    identity.ensure_admin()?;
    let kernel = app.runtime.import_kernel(&input).await?;
    app.record(&identity, "kernel.import", &input.name).await?;
    Ok(Json(kernel))
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
