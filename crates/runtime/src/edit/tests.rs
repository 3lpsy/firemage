use crate::Runtime;
use firemage_wire::{EnvironmentValue, VmAction, VmSpec, VmState};
use serde_json::json;

#[tokio::test]
async fn environment_edit_keeps_stopped_state_with_a_stale_socket_and_updates_boot_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    let spec: VmSpec = serde_json::from_value(json!({
        "name":"environment-edit", "environment":{"REGION":"before"}
    }))
    .unwrap();
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    std::fs::create_dir_all(std::path::Path::new(&row.socket).parent().unwrap()).unwrap();
    let listener = tokio::net::UnixListener::bind(&row.socket).unwrap();
    drop(listener);
    assert!(std::path::Path::new(&row.socket).exists());
    firemage_queries::set_vm_state(&db, row, "stopped", None, None)
        .await
        .unwrap();
    let mut updated_spec = spec.clone();
    updated_spec
        .environment
        .insert("REGION".into(), EnvironmentValue::Plain("after".into()));
    let updated = runtime
        .update(&user.id, &vm.id, updated_spec)
        .await
        .unwrap();
    assert_eq!(updated.state, VmState::Stopped);
    for _ in 0..3 {
        assert_eq!(
            runtime
                .action(&user.id, &vm.id, VmAction::Refresh)
                .await
                .unwrap()
                .state,
            VmState::Stopped
        );
    }
    let saved = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    assert!(saved.pid.is_none() && saved.process_start.is_none());
    let saved: VmSpec = serde_json::from_str(&saved.spec).unwrap();
    // Preparation regenerates its seed from the saved definition on each stopped start.
    let files = runtime.seed_files(&user.id, &saved).await.unwrap();
    let script = files
        .iter()
        .find(|file| file.path == "firemage/environment.sh")
        .unwrap();
    let output = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            &format!("{}\nprintf '%s' \"$REGION\"", script.content),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"after");
}
