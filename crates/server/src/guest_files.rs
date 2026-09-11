use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
};
use firemage_wire::GuestDirectory;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryQuery {
    #[serde(default = "root_inode")]
    inode: u32,
}
fn root_inode() -> u32 {
    2
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadQuery {
    inode: u32,
    #[serde(default)]
    filename: String,
}

pub async fn directory(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<DirectoryQuery>,
) -> Result<Json<GuestDirectory>> {
    let owner = crate::resources::owned_vm(&app, &identity, &id)
        .await?
        .owner_id;
    Ok(Json(
        app.runtime
            .guest_directory(&owner, &id, query.inode)
            .await?,
    ))
}

pub async fn download(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response> {
    let owner = crate::resources::owned_vm(&app, &identity, &id)
        .await?
        .owner_id;
    let disposition = disposition(&query.filename, query.inode)?;
    let (file, size) = app.runtime.guest_download(&owner, &id, query.inode).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CONTENT_LENGTH, size.to_string()),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(file)),
    )
        .into_response())
}

fn disposition(name: &str, inode: u32) -> anyhow::Result<String> {
    anyhow::ensure!(name.len() <= 1024, "download filename is too long");
    let name = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let name = if name.is_empty() || name == "." || name == ".." {
        format!("inode-{inode}")
    } else {
        name.into()
    };
    let encoded = name
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect::<String>();
    Ok(format!(
        "attachment; filename=\"inode-{inode}\"; filename*=UTF-8''{encoded}"
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn filenames_cannot_inject_headers_or_paths() {
        let value = super::disposition("../résult\r\n\".bin", 12).unwrap();
        assert_eq!(
            value,
            "attachment; filename=\"inode-12\"; filename*=UTF-8''r%C3%A9sult%0D%0A%22.bin"
        );
        assert_eq!(
            super::disposition("..", 2).unwrap(),
            "attachment; filename=\"inode-2\"; filename*=UTF-8''inode-2"
        );
        assert!(super::disposition(&"a".repeat(1025), 2).is_err());
    }
}
