use firemage_runtime::Runtime;
use firemage_wire::{Asset, KernelAlias, VmSpec};
use serde_json::json;

#[tokio::test]
async fn catalog_aliases_and_vm_references_preserve_kernel_identity() {
    let directory = tempfile::tempdir().unwrap();
    let kernel_dir = directory.path().join("kernels");
    let catalog = firemage_kernels::Catalog::open(&kernel_dir).unwrap();
    catalog.upload("vmlinux", b"kernel").unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            kernel_dir: Some(kernel_dir.clone()),
            ..Default::default()
        },
    );
    let spec: VmSpec = serde_json::from_value(
        json!({"name":"review", "kernel":{"kind":"local","path":kernel_dir.join("vmlinux")}}),
    )
    .unwrap();
    let vm = runtime.define(&user.id, spec).await.unwrap();
    assert!(matches!(&vm.spec.kernel, Some(Asset::Kernel {name}) if name == "vmlinux"));
    let kernel = runtime
        .alias_kernel(
            "vmlinux",
            &KernelAlias {
                alias: Some("Linux stable".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(kernel.vm_count, 1);
    assert_eq!(kernel.alias.as_deref(), Some("Linux stable"));
    assert!(runtime.delete_kernel("vmlinux").await.is_err());
    let mut spec = vm.spec;
    spec.kernel = Some(Asset::Local {
        path: directory.path().join("private"),
    });
    assert!(
        runtime
            .update(&user.id, &vm.id, spec.clone())
            .await
            .is_err()
    );
    spec.kernel = None;
    runtime.update(&user.id, &vm.id, spec).await.unwrap();
    runtime.delete_kernel("vmlinux").await.unwrap();
    assert!(runtime.kernels().await.unwrap().is_empty());
    assert!(
        firemage_queries::kernel_aliases(&runtime.db)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn creation_and_deletion_cannot_leave_a_new_dangling_reference() {
    let directory = tempfile::tempdir().unwrap();
    let kernel_dir = directory.path().join("kernels");
    firemage_kernels::Catalog::open(&kernel_dir)
        .unwrap()
        .upload("vmlinux", b"kernel")
        .unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            kernel_dir: Some(kernel_dir.clone()),
            ..Default::default()
        },
    );
    let spec: VmSpec = serde_json::from_value(
        json!({"name":"review", "kernel":{"kind":"kernel","name":"vmlinux"}}),
    )
    .unwrap();
    let (created, deleted) = tokio::join!(
        runtime.define(&user.id, spec),
        runtime.delete_kernel("vmlinux")
    );
    assert_ne!(created.is_ok(), deleted.is_ok());
    assert_eq!(created.is_ok(), kernel_dir.join("vmlinux").exists());
}
