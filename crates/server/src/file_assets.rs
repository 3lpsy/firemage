use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use firemage_wire::{FileAsset, FileAssetAlias, FileAssetLimits, FileAssetUpload};

pub async fn limits(_identity: Identity, State(app): State<App>) -> Json<FileAssetLimits> {
    Json(FileAssetLimits {
        max_bytes: app.runtime.config.asset_max_bytes(),
    })
}

pub async fn list(identity: Identity, State(app): State<App>) -> Result<Json<Vec<FileAsset>>> {
    let _guard = app.runtime.lock("file-assets").await;
    Ok(Json(app.runtime.file_assets(&identity.user.id).await?))
}
pub async fn upload(
    identity: Identity,
    State(app): State<App>,
    Query(input): Query<FileAssetUpload>,
    body: Bytes,
) -> Result<Json<FileAsset>> {
    identity.ensure_admin()?;
    let asset = app
        .runtime
        .upload_file_asset(&identity.user.id, input, body)
        .await?;
    app.record(&identity, "asset.upload", &asset.id).await?;
    Ok(Json(asset))
}
pub async fn alias(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<FileAssetAlias>,
) -> Result<Json<FileAsset>> {
    identity.ensure_admin()?;
    let asset = app
        .runtime
        .alias_file_asset(&identity.user.id, &id, &input.alias)
        .await?;
    app.record(&identity, "asset.alias", &id).await?;
    Ok(Json(asset))
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    app.runtime
        .delete_file_asset(&identity.user.id, &id)
        .await?;
    app.record(&identity, "asset.delete", &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn content(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let _guard = app.runtime.lock("file-assets").await;
    let bytes = app
        .runtime
        .file_asset_content(&identity.user.id, &id)
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    ))
}
