use firemage_wire::VmSpec;
use serde_json::json;

fn spec() -> VmSpec {
    serde_json::from_value(json!({"name":"private-image","rootfs":{
        "kind":"oci", "image":format!("registry.example/team/image@sha256:{}", "a".repeat(64)),
        "registry":{"auth":{"kind":"basic","username":"robot","password_secret":"registry-password"},"ca_secret":"registry-ca"}
    }})).unwrap()
}

#[tokio::test]
async fn registry_secrets_are_owner_scoped_and_rechecked_before_edit_and_prepare() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let other = firemage_queries::add_user(&db, "other".into(), None, true, None)
        .await
        .unwrap();
    let runtime = crate::Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().to_owned()),
            ..Default::default()
        },
    );
    let password = "must-never-appear-in-vm-or-error";
    let vault = runtime.secrets().await.unwrap();
    vault
        .put(&owner.id, "registry-password", password)
        .await
        .unwrap();
    vault
        .put(&owner.id, "registry-ca", "test-ca-material")
        .await
        .unwrap();
    let denied = runtime.define(&other.id, spec()).await.unwrap_err();
    assert!(
        denied
            .to_string()
            .contains("registry secret is unavailable")
    );
    assert!(!format!("{denied:#}").contains(password));
    let vm = runtime.define(&owner.id, spec()).await.unwrap();
    assert!(!serde_json::to_string(&vm).unwrap().contains(password));
    let row = firemage_queries::vm(&runtime.db, &owner.id, &vm.id)
        .await
        .unwrap();
    assert!(!row.spec.contains(password));
    vault.delete(&owner.id, "registry-password").await.unwrap();
    assert!(runtime.update(&owner.id, &vm.id, spec()).await.is_err());
    let error = runtime.materialize_assets(&row, &spec()).await.unwrap_err();
    assert!(error.to_string().contains("registry secret is unavailable"));
    assert!(!format!("{error:#}").contains(password));
}

#[tokio::test]
async fn registry_bearer_header_validation_does_not_echo_token() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let runtime = crate::Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    let token = "private-token\r\nInjected: value";
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&user.id, "token", token)
        .await
        .unwrap();
    let access =
        serde_json::from_value(json!({"auth":{"kind":"bearer","token_secret":"token"}})).unwrap();
    let result = runtime.registry_options(&user.id, Some(&access)).await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("invalid token accepted"),
    };
    assert!(!format!("{error:#}").contains(token));
}
