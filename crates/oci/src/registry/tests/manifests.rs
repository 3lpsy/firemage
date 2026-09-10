use super::fixtures::{Server, digest};
use crate::RegistryCredentials;
use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use serde_json::json;

#[tokio::test]
async fn selects_host_platform_and_verifies_the_index_and_manifest() {
    let manifest = serde_json::to_vec(&json!({"schemaVersion":2,
        "config":{"mediaType":"application/vnd.oci.image.config.v1+json","digest":digest(b"config"),"size":6},"layers":[]})).unwrap();
    let manifest_digest = digest(&manifest);
    let index = serde_json::to_vec(&json!({"schemaVersion":2,"manifests":[
        {"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":manifest_digest,"size":manifest.len(),
         "platform":{"os":"linux","architecture":super::super::architecture().unwrap()}},
        {"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":digest(b"other"),"size":5,
         "platform":{"os":"windows","architecture":"amd64"}}
    ]})).unwrap();
    let index_digest = digest(&index);
    let server = Server::start(
        Router::new()
            .route(
                &format!("/v2/test/image/manifests/{index_digest}"),
                get(move || {
                    let bytes = index.clone();
                    async move { bytes }
                }),
            )
            .route(
                &format!("/v2/test/image/manifests/{manifest_digest}"),
                get(move || {
                    let bytes = manifest.clone();
                    async move { bytes }
                }),
            ),
    )
    .await;
    let mut registry = server.registry(&index_digest, RegistryCredentials::Anonymous);
    assert!(registry.manifest().await.unwrap().layers().is_empty());
    let corrupt =
        Server::start(Router::new().fallback(get(|| async { b"corrupt".as_slice() }))).await;
    let mut registry = corrupt.registry(&index_digest, RegistryCredentials::Anonymous);
    assert!(
        registry
            .manifest()
            .await
            .unwrap_err()
            .to_string()
            .contains("digest mismatch")
    );
}

#[tokio::test]
async fn streams_are_bounded_without_content_length() {
    let server = Server::start(Router::new().fallback(get(|| async {
        let chunks =
            futures_util::stream::iter([Ok::<_, std::io::Error>(vec![0u8; 8]), Ok(vec![0u8; 8])]);
        axum::body::Body::from_stream(chunks).into_response()
    })))
    .await;
    let mut registry = server.registry(&digest(b"x"), RegistryCredentials::Anonymous);
    let response = registry.get(registry.origin.clone(), false).await.unwrap();
    assert!(response.content_length().is_none());
    assert!(
        super::super::download::read_bounded(response, 10)
            .await
            .unwrap_err()
            .to_string()
            .contains("size limit")
    );
}

#[tokio::test]
async fn blob_hash_and_declared_size_are_checked() {
    let server =
        Server::start(Router::new().fallback(get(|| async { (StatusCode::OK, "wrong") }))).await;
    let mut registry = server.registry(&digest(b"right"), RegistryCredentials::Anonymous);
    let descriptor = serde_json::from_value(json!({"mediaType":"application/vnd.oci.image.layer.v1.tar","digest":digest(b"right"),"size":5})).unwrap();
    assert!(
        registry
            .blob_bytes(&descriptor, 1024)
            .await
            .unwrap_err()
            .to_string()
            .contains("digest mismatch")
    );
}

#[tokio::test]
async fn native_import_keeps_guest_metadata_inside_private_staging() {
    use std::os::unix::fs::PermissionsExt;
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_uid(rustix::process::geteuid().as_raw().into());
    header.set_gid(rustix::process::getegid().as_raw().into());
    header.set_size(3);
    header.set_mode(0o755);
    header.set_cksum();
    builder
        .append_data(&mut header, "bin/sh", &b"sh!"[..])
        .unwrap();
    let layer = builder.into_inner().unwrap();
    let config = serde_json::to_vec(
        &json!({"architecture":super::super::architecture().unwrap(),"os":"linux",
        "rootfs":{"type":"layers","diff_ids":[digest(&layer)]},"config":{"Cmd":["/bin/sh"]}}),
    )
    .unwrap();
    let manifest = serde_json::to_vec(&json!({"schemaVersion":2,
        "config":{"mediaType":"application/vnd.oci.image.config.v1+json","size":config.len(),"digest":digest(&config)},
        "layers":[{"mediaType":"application/vnd.oci.image.layer.v1.tar","size":layer.len(),"digest":digest(&layer)}]})).unwrap();
    let pin = digest(&manifest);
    let server = Server::start(
        Router::new()
            .route(
                &format!("/v2/test/image/manifests/{pin}"),
                get(move || {
                    let bytes = manifest.clone();
                    async move { bytes }
                }),
            )
            .route(
                &format!("/v2/test/image/blobs/{}", digest(&config)),
                get(move || {
                    let bytes = config.clone();
                    async move { bytes }
                }),
            )
            .route(
                &format!("/v2/test/image/blobs/{}", digest(&layer)),
                get(move || {
                    let bytes = layer.clone();
                    async move { bytes }
                }),
            ),
    )
    .await;
    let parent = tempfile::tempdir().unwrap();
    let image = crate::unpack(
        &server.image(&pin),
        parent.path(),
        &crate::RegistryOptions {
            ca_pem: Some(server.pem.clone()),
            ..Default::default()
        },
        16 * 1024 * 1024,
    )
    .await
    .unwrap();
    assert!(image.has_file("/bin/sh").unwrap());
    let outer = image.rootfs().parent().unwrap().to_owned();
    assert_eq!(
        outer.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    image.finalize().unwrap();
    assert_eq!(
        outer.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        image
            .rootfs()
            .join("bin/sh")
            .metadata()
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    drop(image);
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}
