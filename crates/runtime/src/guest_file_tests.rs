use super::*;

#[tokio::test]
async fn browsing_requires_owner_stopped_state_and_regular_root_disk() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            allow_trusted_vms: Some(true),
            ..Default::default()
        },
    );
    let spec: VmSpec =
        serde_json::from_value(serde_json::json!({"name":"files", "security":{"mode":"trusted"}}))
            .unwrap();
    let vm = runtime.define(&user.id, spec).await.unwrap();
    assert!(runtime.guest_directory("other", &vm.id, 2).await.is_err());
    assert!(runtime.guest_download("other", &vm.id, 2).await.is_err());
    assert!(runtime.guest_directory(&user.id, &vm.id, 0).await.is_err());
    let error = runtime
        .guest_directory(&user.id, &vm.id, 2)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("stopped VM"), "{error}");
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    firemage_queries::set_vm_state(&db, row, "stopped", None, None)
        .await
        .unwrap();
    let disk = runtime.directory(&vm.id).join("rootfs.ext4");
    std::fs::create_dir_all(disk.parent().unwrap()).unwrap();
    let target = directory.path().join("outside");
    std::fs::write(&target, b"private host file").unwrap();
    std::os::unix::fs::symlink(&target, &disk).unwrap();
    assert!(runtime.guest_directory(&user.id, &vm.id, 2).await.is_err());
    assert!(runtime.guest_download(&user.id, &vm.id, 2).await.is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"private host file");
}
