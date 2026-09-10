use crate::Runtime;
use firemage_wire::VmSpec;
use serde_json::json;

#[tokio::test]
async fn file_limit_covers_disks_and_snapshot_memory() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let mut spec: VmSpec =
        serde_json::from_value(json!({"name":"size", "memory_mib":256})).unwrap();
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    tokio::fs::create_dir_all(runtime.directory(&vm.id))
        .await
        .unwrap();
    let file = tokio::fs::File::create(runtime.directory(&vm.id).join("rootfs.ext4"))
        .await
        .unwrap();
    file.set_len(2 * 1024 * 1024 * 1024).await.unwrap();
    assert_eq!(
        runtime.jail_file_limit(&row, &spec).await.unwrap(),
        2 * 1024 * 1024 * 1024
    );
    spec.security.file_size_mib = Some(1024);
    assert!(runtime.jail_file_limit(&row, &spec).await.is_err());
    file.set_len(16 * 1024 * 1024).await.unwrap();
    spec.security.file_size_mib = None;
    assert_eq!(
        runtime.jail_file_limit(&row, &spec).await.unwrap(),
        320 * 1024 * 1024
    );
}

#[test]
fn managed_snapshots_reject_foreign_paths_traversal_and_symlinks() {
    use super::snapshots::resolve_path;
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path();
    let expected = base.join("snapshot.state");
    assert_eq!(
        resolve_path(base, "snapshot.state").unwrap(),
        expected.to_str().unwrap()
    );
    assert_eq!(
        resolve_path(base, expected.to_str().unwrap()).unwrap(),
        expected.to_str().unwrap()
    );
    for path in [
        "/etc/shadow",
        "../snapshot.state",
        "nested/snapshot.state",
        "../other-vm/snapshots/snapshot.state",
    ] {
        assert!(resolve_path(base, path).is_err(), "{path}");
    }
    std::os::unix::fs::symlink("/etc/shadow", base.join("linked.state")).unwrap();
    assert!(resolve_path(base, "linked.state").is_err());
    std::fs::create_dir(base.join("directory.state")).unwrap();
    assert!(resolve_path(base, "directory.state").is_err());
    std::fs::write(&expected, "snapshot").unwrap();
    assert!(resolve_path(base, "snapshot.state").is_ok());
}

#[tokio::test]
async fn direct_modes_keep_typed_snapshots_in_private_managed_storage() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    for mode in ["trusted", "external"] {
        let row = firemage_queries::insert_vm(
            &db,
            &user.id,
            mode,
            json!({"name":mode,"security":{"mode":mode}}).to_string(),
            format!("/run/{mode}.sock"),
        )
        .await
        .unwrap();
        let (state, memory) = runtime
            .snapshot_paths(&row, "state.bin", "memory.bin")
            .await
            .unwrap();
        let base = runtime.directory(&row.id).join("snapshots");
        assert_eq!(std::path::Path::new(&state).parent().unwrap(), base);
        assert_eq!(std::path::Path::new(&memory).parent().unwrap(), base);
        assert_eq!(
            std::fs::metadata(&base).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(
            runtime
                .snapshot_paths(&row, "/etc/shadow", "memory.bin")
                .await
                .is_err()
        );
        assert!(
            runtime
                .snapshot_paths(&row, "same.bin", "same.bin")
                .await
                .is_err()
        );
    }
}

#[test]
fn filesystem_identity_follows_procfs_objects_without_trusting_link_text() {
    use std::os::fd::AsRawFd;
    let directory = tempfile::tempdir().unwrap();
    let file_path = directory.path().join("resource");
    let file = std::fs::File::create(&file_path).unwrap();
    let descriptor_path = std::path::PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
    let alias = directory.path().join("alias");
    std::fs::hard_link(&file_path, &alias).unwrap();
    assert!(crate::process::is_same_file(&descriptor_path, &alias).unwrap());
    std::fs::remove_file(&file_path).unwrap();
    assert!(crate::process::is_same_file(&descriptor_path, &alias).unwrap());
    let other = directory.path().join("other");
    std::fs::write(&other, b"").unwrap();
    assert!(!crate::process::is_same_file(&descriptor_path, &other).unwrap());
}

#[tokio::test]
async fn restore_requires_original_assets_without_recreating_boot_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    let spec: VmSpec =
        serde_json::from_value(json!({"name":"resume", "userdata":"original script"})).unwrap();
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    let assets = runtime.directory(&vm.id);
    tokio::fs::create_dir_all(&assets).await.unwrap();
    for name in ["kernel", "rootfs.ext4", "seed.ext4"] {
        tokio::fs::write(assets.join(name), b"original bytes")
            .await
            .unwrap();
    }
    runtime.ensure_prepared_assets(&row, &spec).await.unwrap();
    assert_eq!(
        tokio::fs::read(assets.join("seed.ext4")).await.unwrap(),
        b"original bytes"
    );
    tokio::fs::remove_file(assets.join("seed.ext4"))
        .await
        .unwrap();
    assert!(
        runtime
            .ensure_prepared_assets(&row, &spec)
            .await
            .unwrap_err()
            .to_string()
            .contains("original prepared seed.ext4")
    );
    assert!(!assets.join("seed.ext4").exists());
}
