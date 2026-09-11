use super::{compatibility, restore_files::DiskSwap};
use firemage_wire::{SnapshotFile, SnapshotManifest, VmSpec};
use serde_json::{Value, json};

fn manifest() -> SnapshotManifest {
    let spec = serde_json::from_value(json!({"name":"source"})).unwrap();
    SnapshotManifest {
        version: 1,
        source_vm_name: "source".into(),
        architecture: std::env::consts::ARCH.into(),
        firecracker_version: "1.16.1".into(),
        spec,
        network: None,
        gateway_mac: None,
        files: ["state.bin", "memory.bin", "kernel", "rootfs.ext4"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    SnapshotFile {
                        size_bytes: 1,
                        sha256: "a".repeat(64),
                    },
                )
            })
            .collect(),
    }
}
fn config(spec: &VmSpec) -> Value {
    json!({"boot-source":{"kernel_image_path":"/resources/kernel"},"machine-config":{"vcpu_count":spec.vcpus,"mem_size_mib":spec.memory_mib,"smt":false,"huge_pages":"None"},"drives":[{"drive_id":"rootfs","path_on_host":"/resources/rootfs.ext4","is_read_only":false,"is_root_device":true}],"network-interfaces":[]})
}

#[test]
fn restore_allows_names_and_app_changes_but_rejects_hardware_network_mismatch() {
    let manifest = manifest();
    let mut target = manifest.spec.clone();
    target.name = "different-name".into();
    target.environment.insert(
        "NEW".into(),
        serde_json::from_value(json!("value")).unwrap(),
    );
    target.terminal = true;
    compatibility::ensure_target(&manifest, &target).unwrap();
    target.vcpus += 1;
    assert!(compatibility::ensure_target(&manifest, &target).is_err());
    target.vcpus -= 1;
    target.memory_mib += 1;
    assert!(compatibility::ensure_target(&manifest, &target).is_err());
    target.memory_mib -= 1;
    target.network = Some(
        serde_json::from_value(
            json!({"network":"isolated","address":"10.1.0.2","mac":"02:00:00:00:00:01"}),
        )
        .unwrap(),
    );
    assert!(compatibility::ensure_target(&manifest, &target).is_err());
}

#[test]
fn actual_devices_must_use_exact_managed_disks_and_supported_nics() {
    let spec = manifest().spec;
    let valid = config(&spec);
    compatibility::ensure_config(&valid, &spec).unwrap();
    for (field, value) in [
        ("vsock", json!({"guest_cid":3})),
        ("balloon", json!({})),
        ("pmem", json!([{}])),
    ] {
        let mut changed = valid.clone();
        changed[field] = value;
        assert!(
            compatibility::ensure_config(&changed, &spec).is_err(),
            "{field}"
        );
    }
    let mut changed = valid.clone();
    changed["drives"][0]["path_on_host"] = json!("/etc/shadow");
    assert!(compatibility::ensure_config(&changed, &spec).is_err());
    let mut changed = valid.clone();
    changed["drives"]
        .as_array_mut()
        .unwrap()
        .push(valid["drives"][0].clone());
    assert!(compatibility::ensure_config(&changed, &spec).is_err());
    let mut changed = valid.clone();
    changed["network-interfaces"] =
        json!([{"iface_id":"unexpected","guest_mac":"02:00:00:00:00:01"}]);
    assert!(compatibility::ensure_config(&changed, &spec).is_err());
    let mut changed = valid;
    changed["drives"].as_array_mut().unwrap().push(json!({"drive_id":"seed","path_on_host":"/resources/seed.ext4","is_read_only":true,"is_root_device":false}));
    compatibility::ensure_config(&changed, &spec).unwrap();
    changed["drives"][1]["is_read_only"] = json!(false);
    assert!(compatibility::ensure_config(&changed, &spec).is_err());
}

#[test]
fn failed_restore_rolls_back_disks_and_preserves_logs() {
    let directory = tempfile::tempdir().unwrap();
    let staged = tempfile::tempdir_in(directory.path()).unwrap();
    std::fs::write(directory.path().join("rootfs.ext4"), b"original root").unwrap();
    std::fs::write(directory.path().join("seed.ext4"), b"original seed").unwrap();
    std::fs::write(directory.path().join("serial.log"), b"serial history").unwrap();
    std::fs::write(
        directory.path().join("firecracker.log"),
        b"diagnostic history",
    )
    .unwrap();
    std::fs::write(staged.path().join("rootfs.ext4"), b"restored root").unwrap();
    std::fs::write(staged.path().join("kernel"), b"restored kernel").unwrap();
    let mut swap = DiskSwap::install(directory.path(), staged.path(), &manifest()).unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("rootfs.ext4")).unwrap(),
        b"restored root"
    );
    assert!(!directory.path().join("seed.ext4").exists());
    swap.rollback().unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("rootfs.ext4")).unwrap(),
        b"original root"
    );
    assert_eq!(
        std::fs::read(directory.path().join("seed.ext4")).unwrap(),
        b"original seed"
    );
    assert!(!directory.path().join("kernel").exists());
    assert_eq!(
        std::fs::read(directory.path().join("serial.log")).unwrap(),
        b"serial history"
    );
    super::restore_files::preserve_diagnostics(directory.path()).unwrap();
    let history = std::fs::read_dir(directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("firecracker.log.before-restore-")
        })
        .unwrap();
    assert_eq!(
        std::fs::read(history.path()).unwrap(),
        b"diagnostic history"
    );
}

#[test]
fn interrupted_file_install_recovers_existing_resources() {
    let directory = tempfile::tempdir().unwrap();
    let staged = tempfile::tempdir_in(directory.path()).unwrap();
    std::fs::write(directory.path().join("rootfs.ext4"), b"original").unwrap();
    std::fs::write(staged.path().join("kernel"), b"new kernel").unwrap();
    assert!(DiskSwap::install(directory.path(), staged.path(), &manifest()).is_err());
    assert_eq!(
        std::fs::read(directory.path().join("rootfs.ext4")).unwrap(),
        b"original"
    );
    assert!(!directory.path().join("kernel").exists());
}

#[tokio::test]
async fn restored_network_names_are_owner_scoped_and_preserve_saved_policy() {
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let source_owner = firemage_queries::add_user(&db, "source-owner".into(), None, true, None)
        .await
        .unwrap()
        .id;
    let target_owner = firemage_queries::add_user(&db, "target-owner".into(), None, false, None)
        .await
        .unwrap()
        .id;
    let network = json!({"name":"source-network","subnet":"192.0.2.0/24","gateway":"192.0.2.1","policy":{"mode":"firemage-only"}});
    firemage_queries::insert_network(&db, &source_owner, "source-network", network.to_string())
        .await
        .unwrap();
    let mut target_network = network.clone();
    target_network["name"] = json!("target-network");
    let target_row = firemage_queries::insert_network(
        &db,
        &target_owner,
        "target-network",
        target_network.to_string(),
    )
    .await
    .unwrap();
    let runtime = crate::Runtime::new(db.clone(), Default::default());
    let mut saved = manifest();
    saved.network = Some(serde_json::from_value(network).unwrap());
    saved.spec.network = Some(
        serde_json::from_value(
            json!({"network":"source-network","address":"192.0.2.2","mac":"02:aa:00:00:00:02"}),
        )
        .unwrap(),
    );
    let mut target = saved.spec.clone();
    target.network.as_mut().unwrap().network = "target-network".into();
    target.network.as_mut().unwrap().mac = "02:AA:00:00:00:02".into();
    compatibility::ensure_target(&saved, &target).unwrap();
    runtime
        .ensure_snapshot_network(&target_owner, &saved, &target)
        .await
        .unwrap();
    assert!(
        runtime
            .ensure_snapshot_network(&source_owner, &saved, &target)
            .await
            .is_err()
    );
    assert!(
        runtime
            .ensure_snapshot_network(&target_owner, &saved, &saved.spec)
            .await
            .is_err()
    );

    firemage_queries::delete_network(&db, &source_owner, "source-network")
        .await
        .unwrap();
    runtime
        .ensure_snapshot_network(&target_owner, &saved, &target)
        .await
        .unwrap();
    for (field, value) in [
        ("subnet", json!("192.0.2.0/25")),
        ("gateway", json!("192.0.2.254")),
        ("policy", json!({"mode":"unrestricted"})),
    ] {
        let mut changed = target_network.clone();
        changed[field] = value;
        firemage_queries::update_network(&db, target_row.clone(), changed.to_string())
            .await
            .unwrap();
        let error = runtime
            .ensure_snapshot_network(&target_owner, &saved, &target)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("differs from the snapshot"),
            "{field}: {error}"
        );
    }
    saved.network.as_mut().unwrap().policy = firemage_wire::NetworkPolicy::HostOnly {
        address: "198.51.100.10".parse().unwrap(),
    };
    target_network["policy"] = json!({"mode":"host-only","address":"198.51.100.11"});
    firemage_queries::update_network(&db, target_row, target_network.to_string())
        .await
        .unwrap();
    assert!(
        runtime
            .ensure_snapshot_network(&target_owner, &saved, &target)
            .await
            .is_err()
    );

    target.network.as_mut().unwrap().address = "192.0.2.3".parse().unwrap();
    assert!(compatibility::ensure_target(&saved, &target).is_err());
    target.network.as_mut().unwrap().address = "192.0.2.2".parse().unwrap();
    target.network.as_mut().unwrap().mac = "02:aa:00:00:00:03".into();
    assert!(compatibility::ensure_target(&saved, &target).is_err());
    target.network = None;
    assert!(compatibility::ensure_target(&saved, &target).is_err());
}

#[test]
fn snapshots_accept_only_the_assigned_web_terminal_device() {
    let mut manifest = manifest();
    manifest.spec.web_terminal = Some(Default::default());
    let mut actual = config(&manifest.spec);
    actual["vsock"] = json!({"guest_cid":3,"uds_path":"/shell/vsock.sock"});
    compatibility::ensure_config(&actual, &manifest.spec).unwrap();
    actual["vsock"]["uds_path"] = json!("/another.sock");
    assert!(compatibility::ensure_config(&actual, &manifest.spec).is_err());
    let mut target = manifest.spec.clone();
    compatibility::ensure_target(&manifest, &target).unwrap();
    target.web_terminal = None;
    assert!(compatibility::ensure_target(&manifest, &target).is_err());
}
