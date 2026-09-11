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
    stage(&[input], Some("echo ready"), &staging, 1024)
        .await
        .unwrap();
    let mode = std::fs::metadata(staging.join("nested/file.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(
        std::fs::read(staging.join("nested/file.txt")).unwrap(),
        b"private-data"
    );
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
    assert!(
        seed(&[], None, directory.path(), 1024)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!staging.exists());
    assert!(!directory.path().join("seed.ext4").exists());
}

#[tokio::test]
async fn staging_enforces_configured_aggregate_including_userdata() {
    let directory = tempfile::tempdir().unwrap();
    let file = BootFile {
        path: "file".into(),
        content: "data".into(),
        encoding: Default::default(),
        destination: None,
        uid: 0,
        gid: 0,
        mode: 0o600,
    };
    assert_eq!(
        stage(
            std::slice::from_ref(&file),
            Some("ok"),
            &directory.path().join("exact"),
            6
        )
        .await
        .unwrap(),
        6
    );
    assert!(
        stage(&[file], Some("ok"), &directory.path().join("over"), 5)
            .await
            .is_err()
    );
    assert!(
        stage(&[], Some("too big"), &directory.path().join("userdata"), 6)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn staging_accepts_files_above_the_former_32_mib_limit() {
    let directory = tempfile::tempdir().unwrap();
    let length = 32 * 1024 * 1024 + 1;
    let file = BootFile {
        path: "file".into(),
        content: "x".repeat(length),
        encoding: Default::default(),
        destination: None,
        uid: 0,
        gid: 0,
        mode: 0o600,
    };
    assert_eq!(
        stage(
            &[file],
            None,
            &directory.path().join("large"),
            length as u64
        )
        .await
        .unwrap(),
        length as u64
    );
}

#[tokio::test]
async fn guest_helper_is_executable_and_started_by_the_existing_setup_hook() {
    let directory = tempfile::tempdir().unwrap();
    let staging = directory.path().join("enabled");
    let helper = BootFile {
        path: "firemage/firemage-guest".into(),
        content: "helper".into(),
        encoding: Default::default(),
        destination: None,
        uid: 0,
        gid: 0,
        mode: 0o700,
    };
    stage(&[helper], None, &staging, 1024).await.unwrap();
    assert_eq!(
        std::fs::metadata(staging.join("firemage/firemage-guest"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let setup = std::fs::read_to_string(staging.join("firemage/setup.sh")).unwrap();
    assert!(setup.contains("mount -t devpts devpts /dev/pts"));
    assert!(setup.contains("/firemage/input/firemage/firemage-guest </dev/null &"));
    let disabled = directory.path().join("disabled");
    stage(&[], Some("echo ready"), &disabled, 1024)
        .await
        .unwrap();
    assert!(!disabled.join("firemage/firemage-guest").exists());
    assert!(
        !std::fs::read_to_string(disabled.join("firemage/setup.sh"))
            .unwrap()
            .contains("devpts")
    );
}
