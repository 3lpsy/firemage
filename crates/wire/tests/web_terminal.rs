use firemage_wire::{VmSpec, WebTerminal};

#[test]
fn web_terminal_defaults_and_managed_scope() {
    let mut spec: VmSpec =
        serde_json::from_value(serde_json::json!({"name":"shell","web_terminal":{}})).unwrap();
    assert_eq!(
        spec.web_terminal.as_ref().unwrap().command,
        ["/bin/sh", "-i"]
    );
    spec.validate().unwrap();
    spec.socket = Some("/tmp/external.sock".into());
    spec.security.mode = firemage_wire::IsolationMode::External;
    assert!(spec.validate().is_err());
    let old: VmSpec = serde_json::from_value(serde_json::json!({"name":"old"})).unwrap();
    assert!(old.web_terminal.is_none());
    assert!(
        serde_json::to_value(old)
            .unwrap()
            .get("web_terminal")
            .is_none()
    );
}
#[test]
fn shell_command_is_bounded_and_absolute() {
    for command in [
        vec![],
        vec!["sh".into()],
        vec!["/bin/sh\0".into()],
        vec!["/bin/sh".into(), "a".repeat(4097)],
    ] {
        assert!(WebTerminal { command }.validate().is_err());
    }
    WebTerminal {
        command: vec!["/bin/bash".into(), "--noprofile".into(), "-i".into()],
    }
    .validate()
    .unwrap();
}
