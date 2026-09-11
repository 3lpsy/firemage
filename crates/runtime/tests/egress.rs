use firemage_runtime::Runtime;
use firemage_wire::{EnvironmentValue, VmSpec};
use serde_json::json;

#[tokio::test]
async fn definitions_require_owned_secrets_and_firemage_network() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let other = firemage_queries::add_user(&db, "other".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&other.id, "token", "other-secret")
        .await
        .unwrap();
    let mut spec: VmSpec = serde_json::from_value(
        json!({"name":"agent", "environment":{"API_KEY":{"secret":"token"}}}),
    )
    .unwrap();
    assert!(runtime.define(&user.id, spec.clone()).await.is_err());
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&user.id, "token", "owner-secret")
        .await
        .unwrap();
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    let serialized = serde_json::to_string(&vm).unwrap();
    assert!(!serialized.contains("owner-secret"));
    assert!(matches!(
        vm.spec.environment["API_KEY"],
        EnvironmentValue::Secret { .. }
    ));
    spec.name = "proxy".into();
    spec.egress = Some(Default::default());
    assert!(runtime.define(&user.id, spec.clone()).await.is_err());
    spec.network = Some(
        serde_json::from_value(
            json!({"network":"restricted", "address":"172.30.0.2", "mac":"02:00:00:00:00:02"}),
        )
        .unwrap(),
    );
    let network = json!({"name":"restricted", "subnet":"172.30.0.0/24", "gateway":"172.30.0.1", "policy":{"mode":"isolated"}});
    let row = firemage_queries::insert_network(&db, &user.id, "restricted", network.to_string())
        .await
        .unwrap();
    assert!(runtime.define(&user.id, spec.clone()).await.is_err());
    let mut network = network;
    network["policy"]["mode"] = json!("firemage-only");
    firemage_queries::update_network(&db, row, network.to_string())
        .await
        .unwrap();
    runtime.define(&user.id, spec).await.unwrap();
}

#[tokio::test]
async fn firemage_only_prevents_raw_network_bypass_and_unprepared_launch() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let network = json!({"name":"restricted", "subnet":"172.30.0.0/24", "gateway":"172.30.0.1", "policy":{"mode":"firemage-only"}});
    firemage_queries::insert_network(&db, &user.id, "restricted", network.to_string())
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let spec: VmSpec = serde_json::from_value(json!({"name":"locked", "network":{"network":"restricted", "address":"172.30.0.2", "mac":"02:00:00:00:00:02"}})).unwrap();
    let vm = runtime.define(&user.id, spec).await.unwrap();
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    for path in ["/network-interfaces/eth1", "/vsock", "/snapshot/load"] {
        let input = firemage_wire::RawRequest {
            method: "PUT".into(),
            path: path.into(),
            body: None,
        };
        assert!(
            runtime.ensure_raw_request(&row, &input).await.is_err(),
            "{path}"
        );
    }
    let input = firemage_wire::RawRequest {
        method: "GET".into(),
        path: "/network-interfaces/eth0".into(),
        body: None,
    };
    runtime.ensure_raw_request(&row, &input).await.unwrap();
    let input = firemage_wire::RawRequest {
        method: "PATCH".into(),
        path: "/vm".into(),
        body: None,
    };
    runtime.ensure_raw_request(&row, &input).await.unwrap();
    let error = runtime
        .action(&user.id, &vm.id, firemage_wire::VmAction::Launch)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("require prepare or start"));
}

#[tokio::test]
async fn recovery_failure_keeps_management_available_and_records_error() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let spec = json!({"name":"orphaned", "egress":{}, "network":{"network":"missing", "address":"172.30.0.2", "mac":"02:00:00:00:00:02"}});
    let row = firemage_queries::insert_vm(
        &db,
        &user.id,
        "orphaned",
        spec.to_string(),
        dir.path().join("missing.sock").to_str().unwrap().into(),
    )
    .await
    .unwrap();
    let row = firemage_queries::set_vm_state(&db, row, "running", None, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    runtime.recover_egress().await.unwrap();
    let recovered = firemage_queries::vm(&db, &user.id, &row.id).await.unwrap();
    assert!(
        recovered
            .error
            .as_deref()
            .unwrap()
            .contains("VM egress recovery failed")
    );
    assert!(dir.path().join("egress-pending").join(&row.id).is_file());
    assert!(!runtime.is_egress_active(&row.id).await);
    assert!(runtime.refresh(recovered).await.unwrap().error.is_some());
}
