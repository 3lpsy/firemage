use firemage_wire::Asset;

#[tokio::test]
async fn failed_preparation_preserves_existing_disk_and_success_copies_source() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let source = directory.path().join("source.ext4");
    let disk = directory.path().join("rootfs.ext4");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&disk, b"existing").unwrap();
    let missing = Asset::Local {
        path: directory.path().join("missing"),
    };
    assert!(firemage_assets::materialize(&missing, &disk).await.is_err());
    assert_eq!(std::fs::read(&disk).unwrap(), b"existing");
    let unverified = Asset::Remote {
        url: "https://example.test/disk".into(),
        sha256: "not-a-digest".into(),
    };
    assert!(
        firemage_assets::materialize(&unverified, &disk)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&disk).unwrap(), b"existing");
    firemage_assets::materialize(
        &Asset::Local {
            path: source.clone(),
        },
        &disk,
    )
    .await
    .unwrap();
    std::fs::write(&disk, b"guest writes").unwrap();
    assert_eq!(std::fs::read(source).unwrap(), b"source");
    assert!(!disk.with_extension("partial").exists());
}
