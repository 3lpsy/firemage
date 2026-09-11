use super::*;

#[test]
fn command_arguments_survive_shell_metacharacters() {
    let literal = "a 'quote' $HOME `id` ; $(id)\nsecond line";
    let workload = firemage_wire::Workload {
        mode: WorkloadMode::OneShot,
        command: Some(vec!["printf".into(), "%s".into(), literal.into()]),
    };
    workload.validate().unwrap();
    let output = std::process::Command::new("/bin/sh")
        .args(["-c", &(script(&workload) + "\"$@\"\n")])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, literal.as_bytes());
}

#[test]
fn workload_rejects_invalid_command_and_non_oci_guests() {
    for command in [
        serde_json::json!([]),
        serde_json::json!([""]),
        serde_json::json!(["a\0b"]),
    ] {
        let workload: firemage_wire::Workload =
            serde_json::from_value(serde_json::json!({"mode":"one-shot", "command":command}))
                .unwrap();
        assert!(workload.validate().is_err());
    }
    let mut spec: VmSpec = serde_json::from_value(serde_json::json!({
        "name":"review", "workload":{"mode":"one-shot"}
    }))
    .unwrap();
    assert!(
        spec.validate()
            .unwrap_err()
            .to_string()
            .contains("managed OCI")
    );
    spec.rootfs = Some(firemage_wire::Asset::Oci {
        image: "example/runner:stable".into(),
        size_mib: 2048,
        registry: None,
    });
    spec.validate().unwrap();
}

#[tokio::test]
async fn prepared_legacy_disk_allows_image_job_but_rejects_unsupported_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let mut spec: VmSpec = serde_json::from_value(serde_json::json!({
        "name":"review", "rootfs":{"kind":"oci","image":"example/runner:stable"},
        "workload":{"mode":"one-shot"}
    }))
    .unwrap();
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    let directory = runtime.directory(&vm.id);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("rootfs.ext4"), b"existing guest disk").unwrap();
    runtime
        .update(&user.id, &vm.id, spec.clone())
        .await
        .unwrap();
    spec.workload.as_mut().unwrap().command = Some(vec!["/bin/true".into()]);
    assert!(
        runtime
            .update(&user.id, &vm.id, spec.clone())
            .await
            .unwrap_err()
            .to_string()
            .contains("create a new VM")
    );
    std::fs::write(directory.join("oci-init-version"), b"1\n").unwrap();
    runtime
        .update(&user.id, &vm.id, spec.clone())
        .await
        .unwrap();
    let files = runtime.seed_files(&user.id, &spec).await.unwrap();
    assert!(
        files.iter().any(|file| file.path == "firemage/workload.sh"
            && file.content.contains("set -- '/bin/true'"))
    );
    assert_eq!(
        std::fs::read(directory.join("rootfs.ext4")).unwrap(),
        b"existing guest disk"
    );
}

#[test]
fn web_terminal_uses_existing_managed_setup_without_replacing_disks() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = crate::Runtime::new(
        firemage_queries::DatabaseConnection::Disconnected,
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let mut spec: firemage_wire::VmSpec = serde_json::from_value(serde_json::json!({"name":"shell","rootfs":{"kind":"oci","image":"example:test"},"web_terminal":{}})).unwrap();
    assert!(runtime.ensure_web_terminal_init("vm", &spec).is_ok());
    let directory = runtime.directory("vm");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("rootfs.ext4"), b"existing").unwrap();
    assert!(runtime.ensure_web_terminal_init("vm", &spec).is_err());
    spec.web_terminal = None;
    assert!(runtime.ensure_web_terminal_init("vm", &spec).is_ok());
    spec.web_terminal = Some(Default::default());
    std::fs::write(directory.join("oci-init-version"), b"1\n").unwrap();
    assert!(runtime.ensure_web_terminal_init("vm", &spec).is_ok());
    assert_eq!(
        std::fs::read(directory.join("rootfs.ext4")).unwrap(),
        b"existing"
    );
}
