use crate::Runtime;
use base64::Engine;
use firemage_wire::{FileAssetUpload, VmSpec, VmState};
use serde_json::json;

async fn fixture() -> (tempfile::TempDir, Runtime, String, String) {
    let directory = tempfile::tempdir().unwrap();
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
            data_dir: Some(directory.path().into()),
            asset_dir: Some(directory.path().join("assets")),
            kernel_dir: Some(directory.path().join("kernels")),
            ..Default::default()
        },
    );
    (directory, runtime, owners.remove(0), owners.remove(0))
}

fn upload(alias: &str) -> FileAssetUpload {
    FileAssetUpload {
        alias: alias.into(),
        filename: "config.json".into(),
    }
}

fn attached(id: &str) -> VmSpec {
    serde_json::from_value(json!({
        "name": "review",
        "attachments": [{"asset_id": id, "destination": "/root/.config/review.json",
                         "uid": 1000, "gid": 1001, "mode": 384}]
    }))
    .unwrap()
}

#[tokio::test]
async fn aliases_are_unique_per_owner_and_vm_references_survive_rename() {
    let (_directory, runtime, owner, other) = fixture().await;
    let asset = runtime
        .upload_file_asset(&owner, upload("config"), vec![1, 2])
        .await
        .unwrap();
    assert!(
        runtime
            .upload_file_asset(&owner, upload("config"), vec![])
            .await
            .is_err()
    );
    runtime
        .upload_file_asset(&other, upload("config"), vec![])
        .await
        .unwrap();
    assert_eq!(runtime.file_assets(&owner).await.unwrap().len(), 1);
    assert!(runtime.file_asset_content(&other, &asset.id).await.is_err());
    assert!(
        runtime
            .alias_file_asset(&other, &asset.id, "stolen")
            .await
            .is_err()
    );
    assert!(runtime.delete_file_asset(&other, &asset.id).await.is_err());
    assert!(runtime.define(&other, attached(&asset.id)).await.is_err());

    let vm = runtime.define(&owner, attached(&asset.id)).await.unwrap();
    assert_eq!(vm.state, VmState::Defined);
    let renamed = runtime
        .alias_file_asset(&owner, &asset.id, "review-config")
        .await
        .unwrap();
    assert_eq!(renamed.id, asset.id);
    assert_eq!(renamed.vm_count, 1);
    assert!(runtime.delete_file_asset(&owner, &asset.id).await.is_err());
    let mut spec = vm.spec;
    spec.attachments.clear();
    runtime.update(&owner, &vm.id, spec).await.unwrap();
    runtime.delete_file_asset(&owner, &asset.id).await.unwrap();
    assert!(runtime.file_assets(&owner).await.unwrap().is_empty());
    assert!(!runtime.config.asset_dir().join(&asset.id).exists());
}

#[tokio::test]
async fn seed_preserves_binary_content_destination_and_permissions_and_rejects_tampering() {
    let (_directory, runtime, owner, other) = fixture().await;
    let bytes = vec![0, 255, 128, b'\n'];
    let asset = runtime
        .upload_file_asset(&owner, upload("binary"), bytes.clone())
        .await
        .unwrap();
    let spec = attached(&asset.id);
    let files = runtime.seed_files(&owner, &spec).await.unwrap();
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(&file.content)
            .unwrap(),
        bytes
    );
    assert_eq!(
        file.destination.as_deref(),
        Some("/root/.config/review.json")
    );
    assert_eq!((file.uid, file.gid, file.mode), (1000, 1001, 0o600));
    assert!(runtime.seed_files(&other, &spec).await.is_err());
    std::fs::write(runtime.config.asset_dir().join(&asset.id), [1, 2, 3, 4]).unwrap();
    assert!(
        runtime
            .seed_files(&owner, &spec)
            .await
            .unwrap_err()
            .to_string()
            .contains("changed on disk")
    );
}

#[tokio::test]
async fn attachment_destinations_are_validated_on_create_and_edit() {
    let (_directory, runtime, owner, _) = fixture().await;
    let asset = runtime
        .upload_file_asset(&owner, upload("config"), vec![])
        .await
        .unwrap();
    let vm = runtime.define(&owner, attached(&asset.id)).await.unwrap();
    for destination in [
        "relative",
        "/root/../etc/config",
        "/proc/config",
        "/firemage/input/config",
    ] {
        let mut spec = attached(&asset.id);
        spec.attachments[0].destination = destination.into();
        assert!(
            runtime.define(&owner, spec.clone()).await.is_err(),
            "{destination}"
        );
        assert!(
            runtime.update(&owner, &vm.id, spec).await.is_err(),
            "{destination}"
        );
    }
    let mut spec = attached(&asset.id);
    spec.attachments.push(spec.attachments[0].clone());
    assert!(runtime.define(&owner, spec).await.is_err());
    let mut spec = attached(&asset.id);
    spec.attachments[0].mode = 0o4755;
    assert!(runtime.define(&owner, spec).await.is_err());
}

#[tokio::test]
async fn attachment_creation_and_asset_deletion_cannot_leave_a_dangling_reference() {
    let (_directory, runtime, owner, _) = fixture().await;
    let asset = runtime
        .upload_file_asset(&owner, upload("config"), vec![])
        .await
        .unwrap();
    let (created, deleted) = tokio::join!(
        runtime.define(&owner, attached(&asset.id)),
        runtime.delete_file_asset(&owner, &asset.id)
    );
    assert_ne!(created.is_ok(), deleted.is_ok());
    assert_eq!(
        created.is_ok(),
        runtime.config.asset_dir().join(&asset.id).exists()
    );
}
