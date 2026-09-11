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

#[tokio::test]
async fn configured_limit_applies_to_uploads_reads_and_attachment_files() {
    let (_directory, mut runtime, owner, _) = fixture().await;
    runtime.config.asset_max_bytes = Some(4);
    let asset = runtime
        .upload_file_asset(&owner, upload("boundary"), vec![1, 2, 3, 4])
        .await
        .unwrap();
    assert!(
        runtime
            .upload_file_asset(&owner, upload("too-large"), vec![0; 5])
            .await
            .is_err()
    );
    assert_eq!(
        runtime.file_asset_content(&owner, &asset.id).await.unwrap(),
        [1, 2, 3, 4]
    );
    let mut spec = attached(&asset.id);
    spec.userdata = Some("echo ready".into());
    assert_eq!(runtime.seed_files(&owner, &spec).await.unwrap().len(), 1);
    std::fs::write(runtime.config.asset_dir().join(&asset.id), [0; 5]).unwrap();
    assert!(runtime.file_asset_content(&owner, &asset.id).await.is_err());
    assert!(
        runtime
            .validate_asset_attachments(&owner, &spec)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn full_size_assets_fit_boot_budget_without_large_test_allocations() {
    let (_directory, runtime, owner, _) = fixture().await;
    let asset = runtime
        .upload_file_asset(&owner, upload("large"), vec![])
        .await
        .unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(runtime.config.asset_dir().join(&asset.id))
        .unwrap();
    file.set_len(runtime.config.asset_max_bytes()).unwrap();
    firemage_queries::delete_file_asset(&runtime.db, &owner, &asset.id)
        .await
        .unwrap();
    firemage_queries::insert_file_asset(
        &runtime.db,
        firemage_orm::file_assets::Model {
            id: asset.id.clone(),
            owner_id: owner.clone(),
            alias: asset.alias,
            filename: asset.filename,
            storage_name: None,
            size_bytes: runtime.config.asset_max_bytes() as i64,
            sha256: asset.sha256,
            created_at: asset.created_at,
        },
    )
    .await
    .unwrap();
    let mut spec = attached(&asset.id);
    spec.userdata = Some("echo ready".into());
    runtime
        .validate_asset_attachments(&owner, &spec)
        .await
        .unwrap();
    let mut second = spec.attachments[0].clone();
    second.destination = "/root/second-file".into();
    spec.attachments.push(second);
    assert!(
        runtime
            .validate_asset_attachments(&owner, &spec)
            .await
            .unwrap_err()
            .to_string()
            .contains("combined guest boot inputs exceed")
    );
}

#[tokio::test]
async fn lowering_upload_limit_still_allows_deleting_unreferenced_assets() {
    let (_directory, mut runtime, owner, _) = fixture().await;
    let asset = runtime
        .upload_file_asset(&owner, upload("old"), vec![0; 5])
        .await
        .unwrap();
    runtime.config.asset_max_bytes = Some(4);
    runtime.delete_file_asset(&owner, &asset.id).await.unwrap();
    assert!(runtime.file_assets(&owner).await.unwrap().is_empty());
    assert!(!runtime.config.asset_dir().join(asset.id).exists());
}

#[tokio::test]
async fn server_files_register_once_with_unique_aliases_and_keep_their_disk_names() {
    let (_directory, runtime, owner, other) = fixture().await;
    runtime
        .upload_file_asset(&owner, upload("config.json"), b"existing".to_vec())
        .await
        .unwrap();
    let foreign = runtime
        .upload_file_asset(&other, upload("foreign"), b"foreign".to_vec())
        .await
        .unwrap();
    let path = runtime.config.asset_dir().join("config.json");
    std::fs::write(&path, b"server file").unwrap();
    let rows = runtime.refresh_file_assets(&owner).await.unwrap();
    assert_eq!(rows.len(), 2);
    let imported = rows
        .iter()
        .find(|row| row.alias == "config.json-1")
        .unwrap();
    assert_eq!(imported.filename, "config.json");
    assert_eq!(
        runtime
            .file_asset_content(&owner, &imported.id)
            .await
            .unwrap(),
        b"server file"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"server file");
    assert!(
        runtime
            .file_asset_content(&owner, &foreign.id)
            .await
            .is_err()
    );
    assert_eq!(runtime.refresh_file_assets(&owner).await.unwrap(), rows);
    runtime
        .validate_asset_attachments(&owner, &attached(&imported.id))
        .await
        .unwrap();
    runtime
        .delete_file_asset(&owner, &imported.id)
        .await
        .unwrap();
    assert!(!path.exists());
    assert_eq!(runtime.refresh_file_assets(&owner).await.unwrap().len(), 1);
}

#[tokio::test]
async fn server_file_discovery_requires_admin_and_rejects_links() {
    let (directory, runtime, owner, _) = fixture().await;
    let reader = firemage_queries::add_user(&runtime.db, "reader".into(), None, false, None)
        .await
        .unwrap();
    std::fs::create_dir_all(runtime.config.asset_dir()).unwrap();
    let path = runtime.config.asset_dir().join("manual.txt");
    std::fs::write(&path, b"server file").unwrap();
    let outside = directory.path().join("private");
    std::fs::write(&outside, b"private").unwrap();
    std::os::unix::fs::symlink(&outside, runtime.config.asset_dir().join("linked")).unwrap();
    std::fs::hard_link(&outside, runtime.config.asset_dir().join("hardlinked")).unwrap();
    assert!(
        runtime
            .refresh_file_assets(&reader.id)
            .await
            .unwrap()
            .is_empty()
    );
    let rows = runtime.refresh_file_assets(&owner).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].alias, "manual.txt");
    assert!(
        runtime
            .refresh_file_assets(&reader.id)
            .await
            .unwrap()
            .is_empty()
    );
    let input = firemage_wire::FileAssetImport {
        alias: "remote".into(),
        filename: "file".into(),
        url: "https://127.0.0.1/private".into(),
        sha256: None,
    };
    assert!(runtime.import_file_asset(&owner, input).await.is_err());
    assert_eq!(runtime.refresh_file_assets(&owner).await.unwrap(), rows);
}

#[tokio::test]
async fn discovery_never_adopts_orphan_managed_uploads() {
    let (_directory, runtime, owner, other) = fixture().await;
    let registered = runtime
        .upload_file_asset(&owner, upload("registered"), b"owned".to_vec())
        .await
        .unwrap();
    let orphan = uuid::Uuid::new_v4().to_string();
    let path = runtime.config.asset_dir().join(&orphan);
    std::fs::write(&path, b"interrupted private upload").unwrap();
    std::fs::write(
        runtime.config.asset_dir().join("manual.txt"),
        b"server file",
    )
    .unwrap();

    let foreign = runtime.refresh_file_assets(&other).await.unwrap();
    assert_eq!(foreign.len(), 1);
    assert_eq!(foreign[0].filename, "manual.txt");
    assert_eq!(
        runtime.refresh_file_assets(&owner).await.unwrap(),
        vec![registered.clone()]
    );
    for admin in [&owner, &other] {
        assert!(runtime.file_asset_content(admin, &orphan).await.is_err());
    }
    assert_eq!(
        runtime
            .file_asset_content(&owner, &registered.id)
            .await
            .unwrap(),
        b"owned"
    );
    assert_eq!(std::fs::read(path).unwrap(), b"interrupted private upload");
}
