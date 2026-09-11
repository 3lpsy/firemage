use crate::Runtime;
use firemage_wire::{SnapshotManifest, SnapshotUpload};
use serde_json::json;

fn archive(path: &std::path::Path) -> std::path::PathBuf {
    let stage = tempfile::tempdir_in(path).unwrap();
    for name in ["state.bin", "kernel", "rootfs.ext4"] {
        std::fs::write(stage.path().join(name), name).unwrap();
    }
    firemage_snapshots::create_private(&stage.path().join("memory.bin"))
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    let files = ["state.bin", "memory.bin", "kernel", "rootfs.ext4"]
        .into_iter()
        .map(|name| (name.to_owned(), json!({"size_bytes":0,"sha256":""})))
        .collect::<serde_json::Map<_, _>>();
    let manifest: SnapshotManifest = serde_json::from_value(json!({
        "version":1,"source_vm_name":"original","architecture":std::env::consts::ARCH,"firecracker_version":"1.16.1",
        "spec":{"name":"original","memory_mib":64}, "network":null,"gateway_mac":null,"files":files
    })).unwrap();
    let output = path.join(format!("{}.fmsnap", uuid::Uuid::new_v4()));
    firemage_snapshots::pack(stage.path(), manifest, &output, 128 * 1024 * 1024).unwrap();
    output
}

#[tokio::test]
async fn catalog_scopes_ownership_verifies_integrity_and_retains_or_deletes_vm_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap()
        .id;
    let other = firemage_queries::add_user(&db, "other".into(), None, true, None)
        .await
        .unwrap()
        .id;
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let input = |alias: &str| SnapshotUpload {
        alias: alias.into(),
        trusted: false,
    };
    let upload = runtime
        .import_snapshot(&owner, input("uploaded"), &archive(dir.path()))
        .await
        .unwrap();
    assert!(upload.source_vm_id.is_none());
    assert!(!upload.trusted);
    assert!(runtime.snapshots(&other).await.unwrap().is_empty());
    assert!(runtime.snapshot_download(&other, &upload.id).await.is_err());
    assert!(runtime.trust_snapshot(&other, &upload.id).await.is_err());
    assert!(runtime.delete_snapshot(&other, &upload.id).await.is_err());
    let target = tempfile::tempdir().unwrap();
    assert!(
        runtime
            .verified_snapshot(&owner, &upload.id, target.path())
            .await
            .unwrap_err()
            .to_string()
            .contains("trusted")
    );
    assert!(
        runtime
            .import_snapshot(&owner, input("uploaded"), &archive(dir.path()))
            .await
            .is_err()
    );
    runtime.trust_snapshot(&owner, &upload.id).await.unwrap();
    runtime
        .verified_snapshot(&owner, &upload.id, target.path())
        .await
        .unwrap();
    let bundle = runtime
        .snapshot_directory()
        .unwrap()
        .join(format!("{}.fmsnap", upload.id));
    let old = std::fs::read(&bundle).unwrap();
    std::fs::write(&bundle, b"tampered").unwrap();
    assert!(
        runtime
            .verified_snapshot(&owner, &upload.id, tempfile::tempdir().unwrap().path())
            .await
            .unwrap_err()
            .to_string()
            .contains("integrity")
    );
    std::fs::write(&bundle, old).unwrap();
    for delete_snapshots in [false, true] {
        let vm = runtime
            .define(
                &owner,
                serde_json::from_value(json!({"name":format!("origin-{delete_snapshots}")}))
                    .unwrap(),
            )
            .await
            .unwrap();
        let source = archive(dir.path());
        let manifest =
            firemage_snapshots::inspect(&source, runtime.config.snapshot_max_bytes()).unwrap();
        let saved = runtime
            .publish_snapshot(
                &owner,
                &format!("saved-{delete_snapshots}"),
                Some(&vm.id),
                true,
                &source,
                manifest,
            )
            .await
            .unwrap();
        runtime
            .delete_with_snapshots(&owner, &vm.id, delete_snapshots)
            .await
            .unwrap();
        assert_eq!(
            runtime.snapshot(&owner, &saved.id).await.is_err(),
            delete_snapshots
        );
        assert_eq!(
            runtime
                .snapshot_directory()
                .unwrap()
                .join(format!("{}.fmsnap", saved.id))
                .exists(),
            !delete_snapshots
        );
    }
    assert!(runtime.snapshot(&owner, &upload.id).await.is_ok());
    runtime.delete_snapshot(&owner, &upload.id).await.unwrap();
    assert!(!bundle.exists());
}
