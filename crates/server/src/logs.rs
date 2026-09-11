use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stream {
    Serial,
    Firecracker,
    Legacy,
}

#[derive(Default, Deserialize)]
pub struct LogQuery {
    stream: Option<Stream>,
    offset: Option<u64>,
}

pub async fn read(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<LogQuery>,
) -> Result<Json<Value>> {
    crate::resources::owned_vm(&app, &identity, &id).await?;
    let directory = app.runtime.directory(&id);
    let legacy_available = directory.join("console.log").is_file();
    let stream = query.stream.unwrap_or_else(|| {
        if !directory.join("serial.log").is_file() && legacy_available {
            Stream::Legacy
        } else {
            Stream::Serial
        }
    });
    let name = match stream {
        Stream::Serial => "serial.log",
        Stream::Firecracker => "firecracker.log",
        Stream::Legacy => "console.log",
    };
    let (bytes, offset, reset) = tail(&directory.join(name), query.offset).await?;
    let stderr = if matches!(stream, Stream::Firecracker) {
        tail(&directory.join("firecracker-stderr.log"), None)
            .await?
            .0
    } else {
        Vec::new()
    };
    use base64::Engine;
    Ok(Json(
        json!({"text":String::from_utf8_lossy(&bytes), "stderr":String::from_utf8_lossy(&stderr), "base64":base64::engine::general_purpose::STANDARD.encode(&bytes), "offset":offset, "reset":reset, "stream":stream, "legacy_available":legacy_available}),
    ))
}

async fn tail(path: &std::path::Path, offset: Option<u64>) -> anyhow::Result<(Vec<u8>, u64, bool)> {
    let mut file = match tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .await
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), 0, offset.is_some_and(|value| value > 0)));
        }
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata().await?;
    anyhow::ensure!(metadata.is_file(), "VM log must be a regular file");
    let reset = offset.is_some_and(|value| value > metadata.len());
    let start = offset
        .filter(|_| !reset)
        .unwrap_or_else(|| metadata.len().saturating_sub(1024 * 1024));
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024).read_to_end(&mut bytes).await?;
    let next = start + bytes.len() as u64;
    Ok((bytes, next, reset))
}
