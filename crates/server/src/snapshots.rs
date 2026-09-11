use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use firemage_wire::{Snapshot, SnapshotLimits, SnapshotRestore, SnapshotSave, SnapshotUpload, Vm};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

pub async fn list(identity: Identity, State(app): State<App>) -> Result<Json<Vec<Snapshot>>> {
    Ok(Json(if identity.user.admin {
        app.runtime.snapshots_all().await?
    } else {
        app.runtime.snapshots(&identity.user.id).await?
    }))
}

async fn owned_snapshot(app: &App, identity: &Identity, id: &str) -> Result<Snapshot> {
    Ok(if identity.user.admin {
        app.runtime.snapshot_by_id(id).await?
    } else {
        app.runtime.snapshot(&identity.user.id, id).await?
    })
}
pub async fn limits(identity: Identity, State(app): State<App>) -> Result<Json<SnapshotLimits>> {
    identity.ensure_admin()?;
    Ok(Json(SnapshotLimits {
        max_bytes: app.runtime.config.snapshot_max_bytes(),
    }))
}
pub async fn upload(
    identity: Identity,
    State(app): State<App>,
    Query(input): Query<SnapshotUpload>,
    body: Body,
) -> Result<Json<Snapshot>> {
    identity.ensure_admin()?;
    firemage_wire::ensure_asset_alias(&input.alias)?;
    let temporary = tempfile::Builder::new()
        .prefix("upload-")
        .tempfile_in(app.runtime.snapshot_directory()?)
        .map_err(anyhow::Error::from)?;
    let mut output = tokio::fs::File::from_std(temporary.reopen().map_err(anyhow::Error::from)?);
    let mut stream = body.into_data_stream();
    let limit = app.runtime.config.snapshot_max_bytes();
    let mut size = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(anyhow::Error::from)?;
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("snapshot size overflow"))?;
        if size > limit {
            return Err(crate::error::Error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "snapshot exceeds configured upload limit".into(),
            ));
        }
        output
            .write_all(&chunk)
            .await
            .map_err(anyhow::Error::from)?;
    }
    output.sync_all().await.map_err(anyhow::Error::from)?;
    drop(output);
    let snapshot = app
        .runtime
        .import_snapshot(&identity.user.id, input, temporary.path())
        .await?;
    app.record(&identity, "snapshot.upload", &snapshot.id)
        .await?;
    Ok(Json(snapshot))
}
pub async fn download(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Response> {
    let snapshot = owned_snapshot(&app, &identity, &id).await?;
    let (file, snapshot) = app
        .runtime
        .snapshot_download(&snapshot.owner_id, &id)
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.fmsnap\"", snapshot.id),
            ),
            (header::CONTENT_LENGTH, snapshot.size_bytes.to_string()),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(file)),
    )
        .into_response())
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    let snapshot = owned_snapshot(&app, &identity, &id).await?;
    app.runtime.delete_snapshot(&snapshot.owner_id, &id).await?;
    app.record(&identity, "snapshot.delete", &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn trust(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<Snapshot>> {
    identity.ensure_admin()?;
    let snapshot = owned_snapshot(&app, &identity, &id).await?;
    let snapshot = app.runtime.trust_snapshot(&snapshot.owner_id, &id).await?;
    app.record(&identity, "snapshot.trust", &id).await?;
    Ok(Json(snapshot))
}
pub async fn save(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<SnapshotSave>,
) -> Result<Json<Snapshot>> {
    identity.ensure_admin()?;
    let owner = crate::resources::owned_vm(&app, &identity, &id)
        .await?
        .owner_id;
    let snapshot = app.runtime.save_snapshot(&owner, &id, &input.alias).await?;
    app.record(&identity, "snapshot.save", &snapshot.id).await?;
    Ok(Json(snapshot))
}
pub async fn restore(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<SnapshotRestore>,
) -> Result<Json<Vm>> {
    identity.ensure_admin()?;
    let snapshot = owned_snapshot(&app, &identity, &input.snapshot_id).await?;
    let owner = crate::resources::owned_vm(&app, &identity, &id)
        .await?
        .owner_id;
    let vm = app
        .runtime
        .restore_snapshot_from(&owner, &id, &snapshot.owner_id, &input.snapshot_id)
        .await?;
    app.record(&identity, "snapshot.restore", &id).await?;
    Ok(Json(vm))
}
