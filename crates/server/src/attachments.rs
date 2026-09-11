use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
};
use firemage_wire::VmSpec;
use serde::Serialize;

#[derive(Serialize)]
pub struct Attachment {
    kind: &'static str,
    name: String,
    destination: String,
    uid: u32,
    gid: u32,
    mode: u32,
}

pub async fn list(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Attachment>>> {
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    let spec: VmSpec = serde_json::from_str(&row.spec).map_err(anyhow::Error::from)?;
    let _guard = app.runtime.lock("file-assets").await;
    let mut entries = Vec::new();
    for file in spec.attachments {
        let asset =
            firemage_queries::file_asset(&app.runtime.db, &row.owner_id, &file.asset_id).await?;
        entries.push(Attachment {
            kind: "Asset",
            name: asset.alias,
            destination: file.destination,
            uid: file.uid,
            gid: file.gid,
            mode: file.mode,
        });
    }
    for file in spec.secret_attachments {
        entries.push(Attachment {
            kind: "Secret",
            name: file.secret,
            destination: file.destination,
            uid: file.uid,
            gid: file.gid,
            mode: file.mode,
        });
    }
    for file in spec.files {
        entries.push(Attachment {
            kind: "Boot file",
            name: file.path,
            destination: file.destination.unwrap_or_else(|| "Seed disk only".into()),
            uid: file.uid,
            gid: file.gid,
            mode: file.mode,
        });
    }
    Ok(Json(entries))
}
