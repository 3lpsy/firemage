use super::bind;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn socket_publication_permissions_and_restart_keep_directory() {
    let directory = tempfile::tempdir().unwrap();
    let parent_inode = std::fs::metadata(directory.path()).unwrap().ino();
    let path = directory.path().join("firemage.sock");
    // SAFETY: getegid has no arguments or memory safety preconditions.
    let gid = unsafe { libc::getegid() };
    let (listener, guard) = bind(&path, 0o660, Some(gid)).await.unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o660);
    assert_eq!(metadata.gid(), gid);
    let mut client = tokio::net::UnixStream::connect(&path).await.unwrap();
    let (mut accepted, _) = listener.accept().await.unwrap();
    client.write_all(b"HTTP").await.unwrap();
    let mut payload = [0; 4];
    accepted.read_exact(&mut payload).await.unwrap();
    assert_eq!(&payload, b"HTTP");
    assert!(bind(&path, 0o600, None).await.is_err());
    drop(listener);
    drop(guard);
    assert!(!path.exists());
    let (listener, guard) = bind(&path, 0o600, None).await.unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert_eq!(
        std::fs::metadata(directory.path()).unwrap().ino(),
        parent_inode
    );
    drop(listener);
    drop(guard);
}

#[tokio::test]
async fn stale_recovery_preserves_live_foreign_and_non_socket_paths() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("firemage.sock");
    std::fs::write(&path, "keep").unwrap();
    assert!(bind(&path, 0o600, None).await.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep");
    std::fs::remove_file(&path).unwrap();
    let live = std::os::unix::net::UnixListener::bind(&path).unwrap();
    assert!(bind(&path, 0o600, None).await.is_err());
    assert!(path.exists());
    drop(live);
    let (listener, guard) = bind(&path, 0o600, None).await.unwrap();
    drop(listener);
    drop(guard);
    let target = directory.path().join("other");
    std::fs::write(&target, "keep").unwrap();
    symlink(&target, &path).unwrap();
    assert!(bind(&path, 0o600, None).await.is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
}

#[tokio::test]
async fn cleanup_preserves_replacement_and_parent_must_be_trusted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("firemage.sock");
    let (listener, guard) = bind(&path, 0o600, None).await.unwrap();
    std::fs::remove_file(&path).unwrap();
    let replacement = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let inode = std::fs::metadata(&path).unwrap().ino();
    drop(listener);
    drop(guard);
    assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
    drop(replacement);
    std::fs::remove_file(&path).unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o770)).unwrap();
    assert!(bind(&path, 0o600, None).await.is_err());
    assert!(
        bind(&directory.path().join("missing/socket"), 0o600, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn stale_recovery_and_cleanup_reject_changed_ownership() {
    // This branch exercises foreign ownership when tests run with privilege in CI.
    // SAFETY: geteuid has no arguments or memory safety preconditions.
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("firemage.sock");
    let (listener, guard) = bind(&path, 0o600, None).await.unwrap();
    std::os::unix::fs::chown(&path, Some(65534), None).unwrap();
    drop(listener);
    drop(guard);
    assert!(path.exists());
    assert!(bind(&path, 0o600, None).await.is_err());
}

#[tokio::test]
async fn management_over_shared_socket_still_requires_authentication() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("firemage.sock");
    let (listener, _guard) = bind(&path, 0o600, None).await.unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let runtime = firemage_runtime::Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().to_owned()),
            ..Default::default()
        },
    );
    let router = crate::router(crate::App::new(runtime).unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .unix_socket(path)
        .build()
        .unwrap();
    let response = client.get("http://localhost/v1/vms").send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    let health = client.get("http://localhost/health").send().await.unwrap();
    assert!(health.status().is_success());
    task.abort();
    let _ = task.await;
}
