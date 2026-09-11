use super::*;

#[tokio::test]
async fn connection_uses_assigned_socket_and_saved_command() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let spec: VmSpec = serde_json::from_value(serde_json::json!({
        "name":"shell", "security":{"mode":"trusted"}, "web_terminal":{"command":["/bin/bash","-i"]}
    }))
    .unwrap();
    let row = firemage_queries::insert_vm(
        &db,
        &owner.id,
        "shell",
        serde_json::to_string(&spec).unwrap(),
        "/unused-api.sock".into(),
    )
    .await
    .unwrap();
    let pid = std::process::id() as i32;
    firemage_queries::set_process(&db, &row.id, pid, crate::process::identity(pid).unwrap())
        .await
        .unwrap();
    let row = firemage_queries::vm(&db, &owner.id, &row.id).await.unwrap();
    let row = firemage_queries::set_vm_state(&db, row, "running", None, Some(pid))
        .await
        .unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    runtime.prepare_shell_directory(&row, &spec).await.unwrap();
    let listener = tokio::net::UnixListener::bind(runtime.shell_socket(&row, &spec)).unwrap();
    let guest = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 13];
        stream.read_exact(&mut request).await.unwrap();
        assert_eq!(&request, b"CONNECT 1024\n");
        stream.write_all(b"OK 1234\n").await.unwrap();
    });
    assert!(
        runtime
            .connect_web_shell("another-owner", &row.id)
            .await
            .is_err()
    );
    let (_, command) = runtime.connect_web_shell(&owner.id, &row.id).await.unwrap();
    assert_eq!(command, ["/bin/bash", "-i"]);
    guest.await.unwrap();
    firemage_queries::set_vm_state(&runtime.db, row.clone(), "stopped", None, None)
        .await
        .unwrap();
    assert!(runtime.connect_web_shell(&owner.id, &row.id).await.is_err());
}
