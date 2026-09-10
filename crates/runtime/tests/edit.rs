use firemage_runtime::Runtime;
use firemage_wire::{Asset, VmSpec};
use serde_json::json;

#[tokio::test]
async fn failed_preparation_allows_correcting_only_unmaterialized_disks() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            local_asset_roots: Some(vec!["/".into()]),
            ..Default::default()
        },
    );
    let mut spec: VmSpec = serde_json::from_value(json!({
        "name":"repair", "rootfs":{"kind":"local", "path":"/missing-rootfs"},
        "drives":[
            {"id":"data", "asset":{"kind":"local", "path":"/data-image"}},
            {"id":"scratch", "asset":{"kind":"local", "path":"/missing-scratch"}}
        ]
    }))
    .unwrap();
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    firemage_queries::set_vm_state(&db, row, "failed", Some("missing rootfs".into()), None)
        .await
        .unwrap();
    let vm_dir = runtime.directory(&vm.id);
    tokio::fs::create_dir_all(&vm_dir).await.unwrap();
    tokio::fs::write(vm_dir.join("console.log"), "boot diagnostics")
        .await
        .unwrap();
    tokio::fs::write(vm_dir.join("kernel"), "prepared kernel")
        .await
        .unwrap();
    spec.rootfs = Some(Asset::Local {
        path: "/corrected-rootfs".into(),
    });
    runtime
        .update(&user.id, &vm.id, spec.clone())
        .await
        .unwrap();

    tokio::fs::write(vm_dir.join("rootfs.ext4"), "guest root contents")
        .await
        .unwrap();
    tokio::fs::write(vm_dir.join("drive-data.img"), "guest data contents")
        .await
        .unwrap();
    let mut changed = spec.clone();
    changed.rootfs = Some(Asset::Local {
        path: "/replacement-rootfs".into(),
    });
    assert!(runtime.update(&user.id, &vm.id, changed).await.is_err());
    spec.drives[1].asset = Asset::Local {
        path: "/corrected-scratch".into(),
    };
    runtime
        .update(&user.id, &vm.id, spec.clone())
        .await
        .unwrap();
    let mut changed = spec.clone();
    changed.drives[0].asset = Asset::Local {
        path: "/replacement-data".into(),
    };
    assert!(runtime.update(&user.id, &vm.id, changed).await.is_err());
    spec.drives.remove(0);
    assert!(runtime.update(&user.id, &vm.id, spec).await.is_err());
    assert_eq!(
        tokio::fs::read_to_string(vm_dir.join("rootfs.ext4"))
            .await
            .unwrap(),
        "guest root contents"
    );
    assert_eq!(
        tokio::fs::read_to_string(vm_dir.join("drive-data.img"))
            .await
            .unwrap(),
        "guest data contents"
    );
}
