use super::*;
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
            ..Default::default()
        },
    );
    (directory, runtime, owners.remove(0), owners.remove(0))
}

fn spec() -> VmSpec {
    serde_json::from_value(json!({"name":"secret-files", "secret_attachments":[
        {"secret":"configuration", "destination":"/root/.config/service.json"}
    ]}))
    .unwrap()
}

#[tokio::test]
async fn secret_files_are_owner_scoped_references_until_private_seed_materialization() {
    let (_directory, runtime, owner, other) = fixture().await;
    let value = "{\"token\":\"private-value\",\"literal\":\"$(do-not-execute)\"}\n";
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&owner, "configuration", value)
        .await
        .unwrap();
    let vm = runtime.define(&owner, spec()).await.unwrap();
    let encoded = serde_json::to_string(&vm).unwrap();
    assert!(encoded.contains("configuration"));
    assert!(!encoded.contains("private-value"));
    let stored = firemage_queries::vm(&runtime.db, &owner, &vm.id)
        .await
        .unwrap();
    assert!(!stored.spec.contains("private-value"));
    assert!(runtime.define(&other, spec()).await.is_err());
    assert!(runtime.seed_files(&other, &spec()).await.is_err());
    let files = runtime.seed_files(&owner, &vm.spec).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "firemage/secrets/0");
    assert_eq!(files[0].content, value);
    assert_eq!(
        files[0].destination.as_deref(),
        Some("/root/.config/service.json")
    );
    assert_eq!((files[0].uid, files[0].gid, files[0].mode), (0, 0, 0o600));
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&owner, "configuration", "rotated")
        .await
        .unwrap();
    let mut customized = spec();
    customized.secret_attachments[0].uid = 1000;
    customized.secret_attachments[0].gid = 1001;
    customized.secret_attachments[0].mode = 0o640;
    let files = runtime.seed_files(&owner, &customized).await.unwrap();
    assert_eq!(files[0].content, "rotated");
    assert_eq!(
        (files[0].uid, files[0].gid, files[0].mode),
        (1000, 1001, 0o640)
    );
    runtime
        .secrets()
        .await
        .unwrap()
        .delete(&owner, "configuration")
        .await
        .unwrap();
    assert!(runtime.seed_files(&owner, &vm.spec).await.is_err());
}

#[test]
fn secret_attachment_validation_rejects_unsafe_paths_ownership_modes_and_collisions() {
    for value in [
        json!({"secret":"../bad", "destination":"/etc/config"}),
        json!({"secret":"config", "destination":"relative"}),
        json!({"secret":"config", "destination":"/etc/../config"}),
        json!({"secret":"config", "destination":"/firemage/input/overwritten"}),
        json!({"secret":"config", "destination":"/proc/config"}),
        json!({"secret":"config", "destination":"/etc/config", "mode":4095}),
        json!({"secret":"config", "destination":"/etc/config", "uid":4294967295u32}),
    ] {
        let value: firemage_wire::SecretAttachment = serde_json::from_value(value).unwrap();
        assert!(value.validate().is_err());
    }
    assert!(
        serde_json::from_value::<firemage_wire::SecretAttachment>(json!({
            "secret":"config", "destination":"/etc/config", "value":"inline-plaintext"
        }))
        .is_err()
    );
    let mut duplicate = spec();
    duplicate
        .secret_attachments
        .push(duplicate.secret_attachments[0].clone());
    assert!(duplicate.validate().is_err());
    let mut collision = spec();
    collision.files.push(
        serde_json::from_value(json!({
            "path":"ordinary", "content":"plain", "destination":"/root/.config/service.json"
        }))
        .unwrap(),
    );
    assert!(collision.validate().is_err());
    let mut too_many = spec();
    too_many.secret_attachments = (0..257)
        .map(|index| {
            let mut attachment = spec().secret_attachments.remove(0);
            attachment.destination = format!("/run/secret-{index}");
            attachment
        })
        .collect();
    assert!(too_many.validate().is_err());
}
