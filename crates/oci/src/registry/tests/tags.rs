use super::fixtures::{Server, digest};
use crate::{RegistryOptions, registry::Registry};
use axum::{Router, routing::get};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn manifest(config: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&json!({"schemaVersion":2,
        "config":{"mediaType":"application/vnd.oci.image.config.v1+json","digest":digest(config),"size":config.len()},"layers":[]})).unwrap()
}

fn registry(server: &Server, tag: &str) -> Registry {
    Registry::new(
        &format!(
            "{}/test/image:{tag}",
            server.origin.trim_start_matches("https://")
        ),
        &RegistryOptions {
            ca_pem: Some(server.pem.clone()),
            ..Default::default()
        },
    )
    .unwrap()
}

#[tokio::test]
async fn tagged_manifest_is_resolved_once_and_config_stays_on_its_digest() {
    let original = manifest(b"original");
    let changed = manifest(b"changed");
    let pin = digest(&original);
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = calls.clone();
    let tagged = original.clone();
    let header = pin.clone();
    let server = Server::start(
        Router::new()
            .route(
                "/v2/test/image/manifests/latest",
                get(move || {
                    let body = if handler_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        tagged.clone()
                    } else {
                        changed.clone()
                    };
                    let header = header.clone();
                    async move { ([("docker-content-digest", header)], body) }
                }),
            )
            .route(
                &format!("/v2/test/image/manifests/{pin}"),
                get(move || {
                    let body = original.clone();
                    async move { body }
                }),
            )
            .route(
                &format!("/v2/test/image/blobs/{}", digest(b"original")),
                get(|| async { b"original".as_slice() }),
            ),
    )
    .await;
    let mut registry = registry(&server, "latest");
    let first = registry.manifest().await.unwrap();
    assert_eq!(registry.reference.digest(), Some(pin.as_str()));
    assert_eq!(
        registry.blob_bytes(first.config(), 1024).await.unwrap(),
        b"original"
    );
    let second = registry.manifest().await.unwrap();
    assert_eq!(first.config().digest(), second.config().digest());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn tag_without_digest_header_resolves_from_content() {
    let bytes = manifest(b"config");
    let pin = digest(&bytes);
    let server = Server::start(Router::new().route(
        "/v2/test/image/manifests/stable",
        get(move || {
            let bytes = bytes.clone();
            async move { bytes }
        }),
    ))
    .await;
    let mut registry = registry(&server, "stable");
    registry.manifest().await.unwrap();
    assert_eq!(registry.reference.digest(), Some(pin.as_str()));
}

#[tokio::test]
async fn tag_rejects_mismatched_or_malformed_advertised_digest() {
    for header in [digest(b"wrong"), "sha256:invalid".to_owned()] {
        let server = Server::start(Router::new().route(
            "/v2/test/image/manifests/stable",
            get(move || {
                let header = header.clone();
                async move { ([("docker-content-digest", header)], manifest(b"config")) }
            }),
        ))
        .await;
        let mut registry = registry(&server, "stable");
        assert!(registry.manifest().await.is_err());
        assert!(registry.reference.digest().is_none());
    }
}

#[test]
fn optional_digest_still_rejects_malformed_references() {
    for image in [
        "registry.example/image:",
        "registry.example/image@sha256:bad",
        "https://registry.example/image:latest",
    ] {
        assert!(Registry::new(image, &RegistryOptions::default()).is_err());
    }
    let registry = Registry::new("registry.example/image", &RegistryOptions::default()).unwrap();
    assert_eq!(registry.reference.tag(), Some("latest"));
}
