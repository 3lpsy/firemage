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
