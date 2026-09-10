use std::process::Command;

#[test]
fn infrastructure_command_is_explicitly_opted_in() {
    let output = Command::new(env!("CARGO_BIN_EXE_firemage"))
        .arg("--help")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).contains("tests-infra"),
        cfg!(feature = "tests-infra")
    );
}

#[cfg(feature = "tests-infra")]
#[test]
fn noninteractive_tests_require_confirmation_before_creating_files() {
    let directory = std::env::temp_dir().join(format!("fm-confirm-test-{}", std::process::id()));
    assert!(!directory.exists());
    let output = Command::new(env!("CARGO_BIN_EXE_firemage"))
        .args(["tests-infra", "--suite", "cli", "--results-dir"])
        .arg(&directory)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("require --confirm"));
    assert!(!directory.exists());
}
