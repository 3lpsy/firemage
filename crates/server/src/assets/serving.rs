use crate::{App, error::Error};
use axum::{
    body::Body,
    extract::State,
    http::{Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use std::path::Path;
include!(concat!(env!("OUT_DIR"), "/webui.rs"));

pub(crate) async fn serve(State(app): State<App>, method: Method, uri: Uri) -> Response {
    let path = uri.path();
    if path == "/v1" || path.starts_with("/v1/") {
        return Error(StatusCode::NOT_FOUND, "API route not found".into()).into_response();
    }
    if !matches!(method, Method::GET | Method::HEAD) {
        return Error(
            StatusCode::METHOD_NOT_ALLOWED,
            "GET or HEAD required".into(),
        )
        .into_response();
    }
    let name = path.trim_start_matches('/');
    if name.contains(['%', '\\', '\0']) || name.split('/').any(|part| part == "." || part == "..") {
        return Error(StatusCode::BAD_REQUEST, "invalid asset path".into()).into_response();
    }
    let target = if name.is_empty() { "index.html" } else { name };
    let asset = load(&app, target).await;
    let (name, body) = match asset {
        Some(body) => (target, body),
        None if target == "index.html" => return unavailable(),
        None if Path::new(target).extension().is_none() => match load(&app, "index.html").await {
            Some(body) => ("index.html", body),
            None => return unavailable(),
        },
        None => return Error(StatusCode::NOT_FOUND, "asset not found".into()).into_response(),
    };
    let size = body.len();
    let mut response = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(body)
    }
    .into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, mime(name).parse().unwrap());
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, size.into());
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
    response
}

async fn load(app: &App, name: &str) -> Option<Vec<u8>> {
    if let Some(directory) = &app.runtime.config.webui_dir {
        let root = tokio::fs::canonicalize(directory).await.ok()?;
        let file = tokio::fs::canonicalize(root.join(name)).await.ok()?;
        if !file.starts_with(&root) {
            return None;
        }
        return tokio::fs::read(file).await.ok();
    }
    EMBEDDED
        .iter()
        .find(|(path, _)| *path == name)
        .map(|(_, bytes)| bytes.to_vec())
}

fn unavailable() -> Response {
    Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Web UI bundle unavailable; run just ui-build before building Firemage".into(),
    )
    .into_response()
}
fn mime(name: &str) -> &'static str {
    match Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
