use std::os::unix::fs::PermissionsExt;

fn run(userdata: &str, fail_setup: bool) -> (tempfile::TempDir, std::process::Output) {
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
    std::fs::write(
        base.join("mounts"),
        "sysfs /sys sysfs rw 0 0\ndevtmpfs /dev devtmpfs rw 0 0\n",
    )
    .unwrap();
    std::fs::write(base.join("cmdline"), "root=/dev/vda rw\n").unwrap();
    let process = serde_json::json!({"env":["CHECK_VALUE=image-default"], "args":["/bin/sh", "-c", "printf 'workload\\n' >> \"$EVENTS\""], "cwd":"/"});
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
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "17\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\nfiles\nquoted 'value $literal\n"
    );
}

#[test]
fn setup_failure_skips_userdata_and_workload() {
    let (dir, output) = run("printf 'userdata\\n' >> \"$EVENTS\"\n", true);
    assert_eq!(output.status.code(), Some(23));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "23\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\n"
    );
}

#[test]
fn premounted_filesystems_allow_userdata_before_workload() {
    let (dir, output) = run("printf 'userdata\\n' >> \"$EVENTS\"\n", false);
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("events")).unwrap(),
        "network\nfiles\nuserdata\nworkload\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("firemage/output/exit-code")).unwrap(),
        "0\n"
    );
}
