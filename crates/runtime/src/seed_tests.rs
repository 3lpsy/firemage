#[tokio::test]
async fn environment_values_are_literal_shell_data() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = crate::Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let value =
        "apostrophe ' $(touch SHOULD_NOT_EXIST) `touch ALSO_NOT_EXIST` \"quoted\"\nsecond line";
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&user.id, "token", value)
        .await
        .unwrap();
    let spec = serde_json::from_value(serde_json::json!({"name":"env", "environment":{"PLAIN": value, "SECRET":{"secret":"token"}}})).unwrap();
    let files = runtime.seed_files(&user.id, &spec).await.unwrap();
    let environment = files
        .iter()
        .find(|file| file.path == "firemage/environment.sh")
        .unwrap();
    let script = dir.path().join("env.sh");
    tokio::fs::write(
        &script,
        format!(
            "{}\nprintf '%s\\0%s' \"$PLAIN\" \"$SECRET\"\n",
            environment.content
        ),
    )
    .await
    .unwrap();
    let output = tokio::process::Command::new("/bin/sh")
        .arg(script)
        .current_dir(dir.path())
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, format!("{value}\0{value}").as_bytes());
    assert!(!dir.path().join("SHOULD_NOT_EXIST").exists());
    assert!(!dir.path().join("ALSO_NOT_EXIST").exists());
}
