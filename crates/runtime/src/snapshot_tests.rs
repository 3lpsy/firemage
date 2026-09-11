use super::*;

#[tokio::test]
async fn managed_snapshot_rejects_tampering_foreign_vm_and_changed_network() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let network = json!({"name":"restricted", "subnet":"172.30.0.0/24", "gateway":"172.30.0.1", "policy":{"mode":"firemage-only"}});
    firemage_queries::insert_network(&db, &user.id, "restricted", network.to_string())
        .await
        .unwrap();
    let spec = json!({"name":"test", "network":{"network":"restricted", "address":"172.30.0.2", "mac":"02:00:00:00:00:02"}});
    let row = firemage_queries::insert_vm(
        &db,
        &user.id,
        "test",
        spec.to_string(),
        "/tmp/test.sock".into(),
    )
    .await
    .unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let state = dir.path().join("state");
    let memory = dir.path().join("memory");
    tokio::fs::write(&state, b"snapshot-device-state")
        .await
        .unwrap();
    tokio::fs::write(&memory, b"guest-memory").await.unwrap();
    let state = state.to_str().unwrap();
    let memory = memory.to_str().unwrap();
    assert!(runtime.ensure_snapshot(&row, state, memory).await.is_err());
    runtime.record_snapshot(&row, state, memory).await.unwrap();
    runtime.ensure_snapshot(&row, state, memory).await.unwrap();
    let mut other = row.clone();
    other.id = uuid::Uuid::new_v4().to_string();
    assert!(
        runtime
            .ensure_snapshot(&other, state, memory)
            .await
            .is_err()
    );
    let mut changed = spec;
    changed["network"]["address"] = json!("172.30.0.3");
    other = row.clone();
    other.spec = changed.to_string();
    assert!(
        runtime
            .ensure_snapshot(&other, state, memory)
            .await
            .is_err()
    );
    tokio::fs::write(state, b"different-device-state")
        .await
        .unwrap();
    assert!(runtime.ensure_snapshot(&row, state, memory).await.is_err());
}

#[tokio::test]
async fn snapshot_network_renames_preserve_uuid_provenance_and_legacy_names_cannot_rebind() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let original = json!({"name":"original","subnet":"10.90.1.0/24","gateway":"10.90.1.1","policy":{"mode":"isolated"}});
    let network =
        firemage_queries::insert_network(&db, &owner.id, "original", original.to_string())
            .await
            .unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let spec: VmSpec = serde_json::from_value(json!({"name":"snapshot-rename","network":{
        "network":network.id,"address":"10.90.1.10","mac":"02:00:00:00:00:10"
    }}))
    .unwrap();
    let vm = runtime.define(&owner.id, spec).await.unwrap();
    let row = firemage_queries::vm(&runtime.db, &owner.id, &vm.id)
        .await
        .unwrap();
    let state = dir.path().join("rename-state");
    let memory = dir.path().join("rename-memory");
    std::fs::write(&state, b"snapshot-state").unwrap();
    std::fs::write(&memory, b"snapshot-memory").unwrap();
    let (state, memory) = (state.to_str().unwrap(), memory.to_str().unwrap());
    runtime.record_snapshot(&row, state, memory).await.unwrap();
    let current = runtime.snapshot_record(&row, state, memory).await.unwrap();
    let record_path = runtime.snapshot_record_path(&row.id, &current).unwrap();
    let mut legacy = current.clone();
    legacy.as_object_mut().unwrap().remove("version");
    legacy["network"]["attachment"]["network"] = json!("original");
    firemage_config::write_private(&record_path, &serde_json::to_vec(&legacy).unwrap()).unwrap();
    runtime.ensure_snapshot(&row, state, memory).await.unwrap();
    let mut other_spec: VmSpec = serde_json::from_str(&row.spec).unwrap();
    let mut other_definition = original.clone();
    other_definition["name"] = json!("other-network");
    let other = firemage_queries::insert_network(
        &runtime.db,
        &owner.id,
        "other-network",
        other_definition.to_string(),
    )
    .await
    .unwrap();
    other_spec.network.as_mut().unwrap().network = other.id;
    let mut rebound = row.clone();
    rebound.spec = serde_json::to_string(&other_spec).unwrap();
    assert!(
        runtime
            .ensure_snapshot(&rebound, state, memory)
            .await
            .is_err()
    );
    let mut renamed = original;
    renamed["name"] = json!("renamed");
    firemage_queries::update_network(&runtime.db, network, renamed.to_string())
        .await
        .unwrap();
    assert!(runtime.ensure_snapshot(&row, state, memory).await.is_err());
    firemage_config::write_private(&record_path, &serde_json::to_vec(&current).unwrap()).unwrap();
    runtime.ensure_snapshot(&row, state, memory).await.unwrap();
    assert!(
        runtime
            .ensure_snapshot(&rebound, state, memory)
            .await
            .is_err()
    );
    let mut changed_policy = renamed;
    changed_policy["policy"] = json!({"mode":"unrestricted"});
    let network = firemage_queries::network(&runtime.db, &owner.id, "renamed")
        .await
        .unwrap();
    firemage_queries::update_network(&runtime.db, network, changed_policy.to_string())
        .await
        .unwrap();
    assert!(runtime.ensure_snapshot(&row, state, memory).await.is_err());
}

#[tokio::test]
async fn legacy_snapshot_without_a_network_remains_compatible_and_malformed_record_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let mut expected = json!({"version":2,"network":{"attachment":null,"definition":null}});
    let mut legacy = json!({"network":{"attachment":null,"definition":null}});
    runtime
        .normalize_snapshot_network("unused", &mut legacy, &mut expected)
        .await
        .unwrap();
    assert_eq!(legacy, expected);
    for mut malformed in [
        json!("invalid"),
        json!({"network":{"attachment":"invalid"}}),
    ] {
        let mut expected = json!({"version":2,"network":{"attachment":null,"definition":null}});
        let result = runtime
            .normalize_snapshot_network("unused", &mut malformed, &mut expected)
            .await;
        assert!(result.is_err() || malformed != expected);
    }
}
