use super::*;
#[tokio::test]
async fn stage_files_are_private_and_guest_setup_preserves_numeric_ownership() {
    let directory = tempfile::tempdir().unwrap();
    let staging = directory.path().join("seed");
    let input = BootFile {
        path: "nested/file.txt".into(),
        content: "private-data".into(),
        encoding: Default::default(),
        destination: Some("/home/guest/file.txt".into()),
        uid: 1000,
        gid: 1001,
        mode: 0o640,
    };
    stage(&[input], Some("echo ready"), &staging).await.unwrap();
    let mode = std::fs::metadata(staging.join("nested/file.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(
        std::fs::metadata(staging.join("nested"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let setup = std::fs::read_to_string(staging.join("firemage/setup.sh")).unwrap();
    assert!(setup.contains("chown 1000:1001 '/home/guest/file.txt'"));
    assert!(setup.contains("chmod 640 '/home/guest/file.txt'"));
    assert!(!setup.contains("private-data"));
}
#[tokio::test]
async fn removing_inputs_removes_stale_seed_and_plaintext_staging() {
    let directory = tempfile::tempdir().unwrap();
    let staging = directory.path().join("seed");
    std::fs::create_dir(&staging).unwrap();
    std::fs::write(staging.join("old-secret"), "old").unwrap();
    std::fs::write(directory.path().join("seed.ext4"), "disk").unwrap();
    assert!(seed(&[], None, directory.path()).await.unwrap().is_none());
    assert!(!staging.exists());
    assert!(!directory.path().join("seed.ext4").exists());
}
