use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn process_output_is_separate_and_legacy_history_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let old = directory.path().join("console.log");
    std::fs::write(&old, "mixed historical log").unwrap();
    let logs = ProcessLogs::open(directory.path()).unwrap();
    let result = std::process::Command::new("/bin/sh")
        .args(["-c", "printf 'guest output'; printf 'host diagnostic' >&2"])
        .stdout(logs.serial)
        .stderr(logs.stderr)
        .status()
        .unwrap();
    assert!(result.success());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("serial.log")).unwrap(),
        "guest output"
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("firecracker-stderr.log")).unwrap(),
        "host diagnostic"
    );
    assert_eq!(
        std::fs::read_to_string(old).unwrap(),
        "mixed historical log"
    );
    assert_eq!(
        std::fs::metadata(directory.path().join("serial.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn log_capture_rejects_symlinks_and_non_regular_files() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target");
    std::fs::write(&target, "untouched").unwrap();
    std::os::unix::fs::symlink(&target, directory.path().join("serial.log")).unwrap();
    assert!(ProcessLogs::open(directory.path()).is_err());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "untouched");
    std::fs::remove_file(directory.path().join("serial.log")).unwrap();
    std::fs::create_dir(directory.path().join("serial.log")).unwrap();
    assert!(ProcessLogs::open(directory.path()).is_err());
}
