use crate::report::Report;
use std::process::Command;

#[test]
fn failed_suite_retains_output_and_structured_failure() {
    let directory = tempfile::tempdir().unwrap();
    let mut report = Report::new(directory.path()).unwrap();
    let mut command = Command::new("sh");
    command.args(["-c", "echo guest-output; echo guest-error >&2; exit 7"]);
    assert!(report.run("fixture", &mut command).is_err());
    let result: serde_json::Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("infra-results.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["status"], "failed");
    assert_eq!(result["checks"][0]["exit_code"], 7);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("infra-fixture-stdout.log")).unwrap(),
        "guest-output\n"
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("infra-fixture-stderr.log")).unwrap(),
        "guest-error\n"
    );
}
