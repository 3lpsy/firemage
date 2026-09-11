#[test]
fn redirects_allow_release_handoffs_but_reject_downgrades_credentials_and_private_targets() {
    use crate::download::redirect_target;
    let origin = reqwest::Url::parse("https://example.com/releases/kernel").unwrap();
    let target = redirect_target(
        &origin,
        "https://cdn.example.com/assets/kernel?signature=value",
        0,
    )
    .unwrap();
    assert_eq!(target.host_str(), Some("cdn.example.com"));
    assert_eq!(
        redirect_target(&target, "../vmlinux", 1).unwrap().as_str(),
        "https://cdn.example.com/vmlinux"
    );
    for location in [
        "http://example.com/kernel",
        "https://user:password@example.com/kernel",
        "https://example.com/kernel#fragment",
        "https://127.0.0.1/kernel",
        "https://10.0.0.1/kernel",
        "https://100.64.0.1/kernel",
        "https://169.254.169.254/kernel",
        "https://[::1]/kernel",
        "https://[::ffff:127.0.0.1]/kernel",
        "//192.168.1.1/kernel",
    ] {
        assert!(redirect_target(&origin, location, 0).is_err(), "{location}");
    }
    let mut current = origin;
    for followed in 0..5 {
        current = redirect_target(&current, "/next", followed).unwrap();
    }
    assert!(redirect_target(&current, "/sixth", 5).is_err());
}

#[test]
fn redirect_dns_answers_must_all_be_public_before_a_connection_is_pinned() {
    use crate::download::ensure_addresses;
    let public: std::net::SocketAddr = "93.184.215.14:443".parse().unwrap();
    assert!(ensure_addresses(&[public]).is_ok());
    assert!(ensure_addresses(&[]).is_err());
    for address in [
        "127.0.0.1:443",
        "100.64.0.1:443",
        "169.254.169.254:443",
        "[::1]:443",
        "[fc00::1]:443",
        "[::ffff:127.0.0.1]:443",
        "192.0.2.1:443",
    ] {
        assert!(
            ensure_addresses(&[public, address.parse().unwrap()]).is_err(),
            "{address}"
        );
    }
}

async fn response(bytes: &'static [u8]) -> reqwest::Response {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request).await.unwrap();
        socket.write_all(bytes).await.unwrap();
    });
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://{address}/file"))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn downloads_stream_verify_and_publish_without_overwrite() {
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    let catalog = crate::Directory::open(root.path(), 0, 8).unwrap();
    let hash = hex::encode(Sha256::digest(b"content"));
    let result = crate::download::save_response(&catalog,
        response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\ncon\r\n4\r\ntent\r\n0\r\n\r\n").await,
        Some(&hash.to_uppercase())).await.unwrap();
    assert_eq!(result.size_bytes, 7);
    assert_eq!(result.sha256, hash);
    assert_eq!(std::fs::read(result.file.path()).unwrap(), b"content");
    catalog.publish("asset", result.file).unwrap();
    assert!(catalog.upload("asset", b"changed").is_err());
    let result = crate::download::save_response(
        &catalog,
        response(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").await,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.size_bytes, 0);
}

#[tokio::test]
async fn oversized_or_unverified_downloads_remove_partial_files() {
    let root = tempfile::tempdir().unwrap();
    let catalog = crate::Directory::open(root.path(), 0, 4).unwrap();
    for data in [
        &b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nlarge"[..],
        &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nbig\r\n3\r\nger\r\n0\r\n\r\n"
            [..],
    ] {
        assert!(
            crate::download::save_response(&catalog, response(data).await, None)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
    assert!(
        crate::download::save_response(
            &catalog,
            response(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ndata").await,
            Some(&"0".repeat(64))
        )
        .await
        .is_err()
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    assert!(
        catalog
            .download("https://127.0.0.1/file", None)
            .await
            .is_err()
    );
    assert!(catalog.download("file:///etc/passwd", None).await.is_err());
    assert!(
        catalog
            .download("https://example.com/file", Some("invalid"))
            .await
            .is_err()
    );
}
