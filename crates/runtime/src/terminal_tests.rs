use super::*;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn input_is_opt_in_owned_bounded_and_limited_to_live_child() {
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(db.clone(), Default::default());
    let spec: VmSpec = serde_json::from_value(serde_json::json!({"name":"console"})).unwrap();
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    let input = TerminalInput {
        input: "hello\r\u{3}".into(),
    };
    assert_eq!(
        runtime
            .terminal_status(&user.id, &vm.id)
            .await
            .unwrap()
            .state,
        TerminalState::Disabled
    );
    assert!(
        runtime
            .terminal_input(&user.id, &vm.id, &input)
            .await
            .is_err()
    );
    assert!(
        runtime
            .terminal_status("other-owner", &vm.id)
            .await
            .is_err()
    );
    let mut spec = spec;
    spec.terminal = true;
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    let row = firemage_queries::update_vm_spec(
        &db,
        row,
        spec.name.clone(),
        serde_json::to_string(&spec).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        runtime
            .terminal_status(&user.id, &vm.id)
            .await
            .unwrap()
            .state,
        TerminalState::NotRunning
    );
    firemage_queries::set_vm_state(&db, row, "running", None, None)
        .await
        .unwrap();
    assert_eq!(
        runtime
            .terminal_status(&user.id, &vm.id)
            .await
            .unwrap()
            .state,
        TerminalState::RestartRequired
    );
    let mut child = tokio::process::Command::new("/bin/cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut output = child.stdout.take().unwrap();
    runtime.children.lock().await.insert(vm.id.clone(), child);
    assert_eq!(
        runtime
            .terminal_status(&user.id, &vm.id)
            .await
            .unwrap()
            .state,
        TerminalState::Available
    );
    assert!(
        runtime
            .terminal_input("other-owner", &vm.id, &input)
            .await
            .is_err()
    );
    for invalid in [String::new(), "é".repeat(2049)] {
        assert!(
            runtime
                .terminal_input(&user.id, &vm.id, &TerminalInput { input: invalid })
                .await
                .is_err()
        );
    }
    runtime
        .terminal_input(&user.id, &vm.id, &input)
        .await
        .unwrap();
    let mut bytes = vec![0; input.input.len()];
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        output.read_exact(&mut bytes),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(bytes, input.input.as_bytes());
    let mut children = runtime.children.lock().await;
    children.get_mut(&vm.id).unwrap().kill().await.unwrap();
    children.get_mut(&vm.id).unwrap().wait().await.unwrap();
    drop(children);
    assert!(
        runtime
            .terminal_input(&user.id, &vm.id, &input)
            .await
            .is_err()
    );
    assert_eq!(
        runtime
            .terminal_status(&user.id, &vm.id)
            .await
            .unwrap()
            .state,
        TerminalState::NotRunning
    );
}

#[test]
fn external_socket_cannot_enable_terminal() {
    let spec: VmSpec = serde_json::from_value(serde_json::json!({
        "name":"external", "security":{"mode":"external"},
        "socket":"/run/firecracker.sock", "terminal":true
    }))
    .unwrap();
    assert!(
        spec.validate()
            .unwrap_err()
            .to_string()
            .contains("terminal input requires a managed VM")
    );
}
