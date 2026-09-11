use std::os::unix::fs::PermissionsExt;

fn run(userdata: &str, fail_setup: bool) -> (tempfile::TempDir, std::process::Output) {
    run_workload(userdata, fail_setup, None, true)
}

fn run_workload(
    userdata: &str,
    fail_setup: bool,
    workload: Option<&str>,
    image_command: bool,
) -> (tempfile::TempDir, std::process::Output) {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let input = base.join("firemage/input");
    std::fs::create_dir_all(input.join("firemage")).unwrap();
    let bin = base.join("bin");
    std::fs::create_dir(&bin).unwrap();
    for name in ["mount", "sync", "reboot"] {
        let path = bin.join(name);
        let script = if name == "mount" {
            "#!/bin/sh\nexit 31\n"
        } else if name == "reboot" {
            "#!/bin/sh\nprintf 'reboot\\n' >> \"$EVENTS\"\nexit 0\n"
        } else {
            "#!/bin/sh\nexit 0\n"
        };
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(
        input.join("firemage/network.sh"),
        "printf 'network\\n' >> \"$EVENTS\"\n",
    )
    .unwrap();
    std::fs::write(
        input.join("firemage/environment.sh"),
        "export CHECK_VALUE='quoted '\"'\"'value $literal'\n",
    )
    .unwrap();
    std::fs::write(
        input.join("firemage/setup.sh"),
        if fail_setup {
            "exit 23\n"
        } else {
            "printf 'files\\n' >> \"$EVENTS\"\n"
        },
    )
    .unwrap();
    std::fs::write(input.join("user-data"), userdata).unwrap();
    if let Some(workload) = workload {
        std::fs::write(input.join("firemage/workload.sh"), workload).unwrap();
    }
    std::fs::write(
        base.join("mounts"),
        "sysfs /sys sysfs rw 0 0\ndevtmpfs /dev devtmpfs rw 0 0\n",
    )
    .unwrap();
    std::fs::write(base.join("cmdline"), "root=/dev/vda rw\n").unwrap();
    let mut process = serde_json::json!({"env":["CHECK_VALUE=image-default"], "args":["/bin/sh", "-c", "printf 'workload\\n' >> \"$EVENTS\""], "cwd":"/"});
    if !image_command {
        process["args"] = serde_json::json!([]);
    }
    let script = super::init(&process)
        .unwrap()
        .replace("/proc/mounts", &base.join("mounts").to_string_lossy())
        .replace("/proc/cmdline", &base.join("cmdline").to_string_lossy())
        .replace("/firemage/input", &input.to_string_lossy())
        .replace(
            "/firemage/output",
            &base.join("firemage/output").to_string_lossy(),
        )
        .replace("while :; do sleep 3600; done", "exit \"$status\"");
    let path = base.join("init.sh");
    std::fs::write(&path, script).unwrap();
    let output = std::process::Command::new("/bin/sh")
        .arg(path)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("EVENTS", base.join("events"))
        .output()
        .unwrap();
    (dir, output)
}

#[test]
fn userdata_failure_records_status_and_skips_workload() {
    let (dir, output) = run(
        "printf '%s\\n' \"$CHECK_VALUE\" >> \"$EVENTS\"\nexit 17\n",
        false,
    );
    assert_eq!(output.status.code(), Some(17));
    assert!(String::from_utf8_lossy(&output.stdout).contains("userdata exited with status 17"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "17\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\nfiles\nquoted 'value $literal\nreboot\n"
    );
}

#[test]
fn setup_failure_skips_userdata_and_workload() {
    let (dir, output) = run("printf 'userdata\\n' >> \"$EVENTS\"\n", true);
    assert_eq!(output.status.code(), Some(23));
    assert!(String::from_utf8_lossy(&output.stdout).contains("setup exited with status 23"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "23\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\nreboot\n"
    );
}

#[test]
fn premounted_filesystems_allow_userdata_before_workload() {
    let (dir, output) = run("printf 'userdata\\n' >> \"$EVENTS\"\n", false);
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\nfiles\nuserdata\nworkload\nreboot\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "0\n"
    );
}

#[test]
fn command_override_runs_once_after_userdata_and_records_failure() {
    let (dir, output) = run_workload(
        "printf 'userdata\\n' >> \"$EVENTS\"\n",
        false,
        Some(
            "firemage_workload_mode=one-shot\nset -- /bin/sh -c 'printf \"override\\n\" >> \"$EVENTS\"; exit 42'\n",
        ),
        true,
    );
    assert_eq!(output.status.code(), Some(42));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\nfiles\nuserdata\noverride\nreboot\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "42\n"
    );
    let serial = String::from_utf8(output.stdout).unwrap();
    assert!(serial.contains("[firemage] userdata completed\n[firemage] main program started\n"));
    assert!(serial.contains("[firemage] workload exited with status 42"));
}

#[test]
fn keep_alive_does_not_reboot_or_restart_main_program() {
    let (dir, output) = run_workload("", false, Some("firemage_workload_mode=keep-alive\n"), true);
    assert!(output.status.success());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\nfiles\nworkload\n"
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("guest remains running")
    );
}

#[test]
fn commandless_image_uses_seed_override_and_rejects_removed_override() {
    let (dir, output) = run_workload(
        "",
        false,
        Some("set -- /bin/sh -c 'printf commandless > \"$EVENTS\"; exit 19'\n"),
        false,
    );
    assert_eq!(output.status.code(), Some(19));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "commandlessreboot\n"
    );
    let (dir, output) = run_workload("", false, None, false);
    assert_eq!(output.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&output.stderr).contains("no workload command configured"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "127\n"
    );
}
