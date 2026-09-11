use crate::Runtime;
use firemage_wire::{FileAssetUpload, KernelAlias, VmConfigDocument, VmConfigImport, VmSpec};
use serde_json::json;

async fn fixture() -> (tempfile::TempDir, Runtime, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let mut owners = Vec::new();
    for name in ["owner", "other"] {
        owners.push(
            firemage_queries::add_user(&db, name.into(), None, true, None)
                .await
                .unwrap()
                .id,
        );
    }
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            asset_dir: Some(dir.path().join("files")),
            kernel_dir: Some(dir.path().join("kernels")),
            ..Default::default()
        },
    );
    (dir, runtime, owners.remove(0), owners.remove(0))
}

#[tokio::test]
async fn portable_roundtrip_resolves_owner_aliases_without_exporting_secret_values() {
    let (_dir, runtime, owner, other) = fixture().await;
    std::fs::create_dir_all(runtime.config.kernel_dir()).unwrap();
    std::fs::write(runtime.config.kernel_dir().join("vmlinux-test"), b"kernel").unwrap();
    runtime
        .alias_kernel(
            "vmlinux-test",
            &KernelAlias {
                alias: Some("linux".into()),
            },
        )
        .await
        .unwrap();
    let upload = || FileAssetUpload {
        alias: "source".into(),
        filename: "source.tar.gz".into(),
    };
    let asset = runtime
        .upload_file_asset(&owner, upload(), b"archive".to_vec())
        .await
        .unwrap();
    let other_asset = runtime
        .upload_file_asset(&other, upload(), b"other archive".to_vec())
        .await
        .unwrap();
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&owner, "credentials", "do-not-export-this-value")
        .await
        .unwrap();
    let spec: VmSpec = serde_json::from_value(json!({
        "name":"review", "kernel":{"kind":"kernel","name":"vmlinux-test"},
        "attachments":[{"asset_id":asset.id,"destination":"/opt/source.tar.gz"}],
        "secret_attachments":[{"secret":"credentials","destination":"/root/.config/service/auth.json"}],
        "environment":{"LANG":"C.UTF-8"}, "terminal":true, "web_terminal":{"command":["/bin/bash","-i"]}
    })).unwrap();
    let vm = runtime.define(&owner, spec).await.unwrap();
    let encoded = runtime.export_vm_config(&owner, &vm.id).await.unwrap();
    assert!(encoded.contains("kernel_alias = \"linux\""));
    assert!(!encoded.contains("vmlinux-test"));
    assert!(!encoded.contains(&asset.id));
    assert!(!encoded.contains("do-not-export-this-value"));
    assert!(encoded.contains("secret = \"credentials\""));
    let doc: VmConfigDocument = toml::from_str(&encoded).unwrap();
    assert!(doc.vm.attachments.is_empty());
    assert!(doc.vm.kernel.is_none());
    let input = VmConfigImport {
        toml: encoded,
        name: Some("copy".into()),
    };
    let (restored, preview) = runtime
        .resolve_vm_config(&owner, &input, None)
        .await
        .unwrap();
    assert_eq!(restored.attachments[0].asset_id, asset.id);
    assert_eq!(restored.secret_attachments[0].mode, 0o600);
    assert!(restored.terminal);
    assert_eq!(restored.web_terminal.unwrap().command, ["/bin/bash", "-i"]);
    assert_eq!(preview.name, "copy");
    assert!(
        preview
            .references
            .iter()
            .any(|r| r.kind == "Secret" && r.alias == "credentials")
    );
    assert!(
        runtime
            .resolve_vm_config(&other, &input, None)
            .await
            .is_err()
    );
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&other, "credentials", "separate-owner-value")
        .await
        .unwrap();
    let (restored, _) = runtime
        .resolve_vm_config(&other, &input, None)
        .await
        .unwrap();
    assert_eq!(restored.attachments[0].asset_id, other_asset.id);
    assert!(runtime.export_vm_config(&other, &vm.id).await.is_err());
    assert!(
        runtime
            .resolve_vm_config(&other, &input, Some(&vm.id))
            .await
            .is_err()
    );
    // An older database may have a directory kernel without alias metadata.
    firemage_queries::set_kernel_alias(&runtime.db, "vmlinux-test", None)
        .await
        .unwrap();
    assert!(
        runtime
            .export_vm_config(&owner, &vm.id)
            .await
            .unwrap()
            .contains("kernel_alias = \"vmlinux-test\"")
    );
    assert!(
        runtime
            .resolve_vm_config(&owner, &input, None)
            .await
            .is_err()
    );
}
mod validation {
    use super::super::validation::ensure_portable;
    use super::*;
    use firemage_wire::Asset;
    #[test]
    fn rejects_raw_catalog_ids_host_paths_and_unknown_versions() {
        let make = || {
            serde_json::from_value::<VmConfigDocument>(
                serde_json::json!({"version":1,"kernel_alias":"linux","vm":{"name":"test"}}),
            )
            .unwrap()
        };
        let mut doc = make();
        ensure_portable(&doc).unwrap();
        doc.version = 2;
        assert!(ensure_portable(&doc).is_err());
        doc = make();
        doc.vm.kernel = Some(Asset::Kernel {
            name: "kernel-file".into(),
        });
        assert!(ensure_portable(&doc).is_err());
        doc = make();
        doc.vm.rootfs = Some(Asset::Local {
            path: "/secret/disk".into(),
        });
        assert!(ensure_portable(&doc).is_err());
        doc = make();
        doc.vm.socket = Some("/run/private.sock".into());
        assert!(ensure_portable(&doc).is_err());
    }
}

#[tokio::test]
async fn imports_choose_free_addresses_and_edits_keep_their_own_reservation() {
    let (_dir, runtime, owner, other) = fixture().await;
    firemage_queries::insert_network(
        &runtime.db,
        &owner,
        "review",
        json!({
            "name":"review", "subnet":"10.77.1.0/29", "gateway":"10.77.1.1",
            "policy":{"mode":"host-only","address":"10.77.1.3"}
        })
        .to_string(),
    )
    .await
    .unwrap();
    let net = runtime
        .suggest_network_address(&owner, "review", None)
        .await
        .unwrap();
    assert_eq!(net.address.to_string(), "10.77.1.2");
    let spec: VmSpec = serde_json::from_value(json!({"name":"first","network":net})).unwrap();
    let vm = runtime.define(&owner, spec.clone()).await.unwrap();
    assert!(runtime.define(&owner, spec.clone()).await.is_err());
    runtime.update(&owner, &vm.id, spec).await.unwrap();
    let next = runtime
        .suggest_network_address(&owner, "review", None)
        .await
        .unwrap();
    assert_eq!(next.address.to_string(), "10.77.1.4");
    assert_ne!(next.mac, net.mac);
    assert!(
        runtime
            .suggest_network_address(&other, "review", None)
            .await
            .is_err()
    );
    assert!(
        runtime
            .suggest_network_address(&other, "review", Some(&vm.id))
            .await
            .is_err()
    );
    let input = VmConfigImport {
        toml: runtime.export_vm_config(&owner, &vm.id).await.unwrap(),
        name: Some("second".into()),
    };
    let (imported, preview) = runtime
        .resolve_vm_config(&owner, &input, None)
        .await
        .unwrap();
    assert_eq!(preview.address.as_deref(), Some("10.77.1.4"));
    assert_ne!(imported.network.as_ref().unwrap().mac, net.mac);
    let (edited, _) = runtime
        .resolve_vm_config(&owner, &input, Some(&vm.id))
        .await
        .unwrap();
    assert_eq!(edited.network.as_ref().unwrap().address, net.address);
    assert_eq!(edited.network.as_ref().unwrap().mac, net.mac);
    runtime.define(&owner, imported).await.unwrap();
    assert_eq!(
        runtime
            .suggest_network_address(&owner, "review", None)
            .await
            .unwrap()
            .address
            .to_string(),
        "10.77.1.5"
    );
}

#[tokio::test]
async fn duplication_copies_configuration_references_without_copying_managed_files() {
    let (_dir, runtime, owner, other) = fixture().await;
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&owner, "credentials", "private-value")
        .await
        .unwrap();
    let vm = runtime
        .define(
            &owner,
            serde_json::from_value(json!({
                "name":"original", "memory_mib":768, "terminal":true, "web_terminal":{},
                "secret_attachments":[{"secret":"credentials","destination":"/root/auth.json"}],
                "environment":{"TOKEN":{"secret":"credentials"}}, "userdata":"echo setup"
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    std::fs::create_dir_all(runtime.directory(&vm.id)).unwrap();
    std::fs::write(
        runtime.directory(&vm.id).join("rootfs.ext4"),
        b"original disk",
    )
    .unwrap();
    let duplicate = runtime.duplicate_vm(&owner, &vm.id, "copy").await.unwrap();
    assert_ne!(duplicate.id, vm.id);
    assert_eq!(duplicate.state, firemage_wire::VmState::Defined);
    assert_eq!(duplicate.spec.name, "copy");
    assert_eq!(duplicate.spec.memory_mib, 768);
    assert!(duplicate.spec.terminal);
    assert_eq!(
        duplicate.spec.web_terminal.as_ref().unwrap().command,
        ["/bin/sh", "-i"]
    );
    assert_eq!(duplicate.spec.secret_attachments[0].secret, "credentials");
    assert_eq!(duplicate.spec.userdata.as_deref(), Some("echo setup"));
    assert!(!runtime.directory(&duplicate.id).exists());
    assert!(
        !serde_json::to_string(&duplicate)
            .unwrap()
            .contains("private-value")
    );
    assert!(
        runtime
            .duplicate_vm(&other, &vm.id, "stolen")
            .await
            .is_err()
    );
    assert!(
        runtime
            .duplicate_vm(&owner, &vm.id, "../invalid")
            .await
            .is_err()
    );
}
