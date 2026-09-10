use firemage_runtime::Runtime;
use firemage_wire::{VmAction, VmState};

#[tokio::test]
async fn recovered_stop_uses_persisted_process_identity() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let spec = serde_json::json!({"name":"recovery"});
    let mut row = firemage_queries::insert_vm(
        &db,
        &user.id,
        "recovery",
        spec.to_string(),
        directory.path().join("fc.sock").to_str().unwrap().into(),
    )
    .await
    .unwrap();
    let mut child = tokio::process::Command::new("sleep")
        .arg("60")
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = child.id().unwrap() as i32;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let start = stat
        .rsplit_once(") ")
        .unwrap()
        .1
        .split_whitespace()
        .nth(19)
        .unwrap()
        .to_owned();
    let start = format!(
        "{}:{start}",
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .unwrap()
            .trim()
    );
    firemage_queries::set_process(&db, &row.id, pid, start)
        .await
        .unwrap();
    row = firemage_queries::set_vm_state(&db, row, "running", None, Some(pid))
        .await
        .unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    let stopped = runtime
        .action(&user.id, &row.id, VmAction::Stop)
        .await
        .unwrap();
    assert_eq!(stopped.state, VmState::Stopped);
    assert!(!child.wait().await.unwrap().success());
    assert_eq!(
        runtime
            .action(&user.id, &row.id, VmAction::Stop)
            .await
            .unwrap()
            .state,
        VmState::Stopped
    );
}

#[tokio::test]
async fn failed_launch_persists_error() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            firecracker: Some(directory.path().join("missing-firecracker")),
            ..Default::default()
        },
    );
    let spec = serde_json::from_value(serde_json::json!({"name":"failure"})).unwrap();
    let vm = runtime.define(&user.id, spec).await.unwrap();
    assert!(
        runtime
            .action(&user.id, &vm.id, VmAction::Start)
            .await
            .is_err()
    );
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    // A failed launch with no child is a failure, never a successful run.
    assert_eq!(row.state, "failed");
    assert_eq!(runtime.refresh(row.clone()).await.unwrap().state, "failed");
    assert!(row.error.is_some());
}
