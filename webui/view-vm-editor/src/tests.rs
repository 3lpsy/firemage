use super::spec::{Form, to_toml};
use serde_json::{Value, json};
fn form() -> Form {
    Form {
        name: "runner".into(),
        mode: "managed".into(),
        isolation: "jailed".into(),
        socket: String::new(),
        kernel: "vmlinux".into(),
        source: "local".into(),
        rootfs: "/assets/root.ext4".into(),
        rootfs_sha: String::new(),
        rootfs_size: "2048".into(),
        workload_mode: "one-shot".into(),
        command: String::new(),
        terminal: false,
        web_terminal: false,
        shell_command: String::new(),
        metadata: String::new(),
        registry: Default::default(),
        vcpus: "2".into(),
        memory: "512".into(),
        network: String::new(),
        address: String::new(),
        mac: "06:00:00:00:00:02".into(),
        userdata: String::new(),
        boot_args: "console=ttyS0".into(),
    }
}
#[test]
fn guided_offline_definition_omits_network_and_preserves_resources() {
    let spec = form().spec().unwrap();
    assert!(spec.get("network").is_none());
    assert_eq!(spec["vcpus"], 2);
    assert_eq!(spec["memory_mib"], 512);
    assert_eq!(
        spec["kernel"],
        json!({ "kind" : "kernel", "name" : "vmlinux" })
    );
}
#[test]
fn catalog_kernel_and_oci_reference_are_preserved() {
    let mut input = form();
    input.source = "oci".into();
    input.rootfs = format!("example.test/runner@sha256:{}", "a".repeat(64));
    let spec = input.spec().unwrap();
    assert_eq!(spec["kernel"]["name"], "vmlinux");
    assert_eq!(spec["rootfs"]["kind"], "oci");
    assert_eq!(
        spec["rootfs"]["image"],
        format!("example.test/runner@sha256:{}", "a".repeat(64))
    );
}
#[test]
fn full_toml_preserves_nested_assets_and_skips_absent_options() {
    let mut original = form().spec().unwrap();
    original["metadata"] = Value::Null;
    original["initrd"] = Value::Null;
    original["files"] = json!(
        [{ "path" : "input", "content" : "hello", "encoding" : "utf8" }]
    );
    let text = to_toml(&original).unwrap();
    let decoded: Value = toml::from_str(&text).unwrap();
    assert_eq!(decoded["kernel"], original["kernel"]);
    assert_eq!(decoded["files"], original["files"]);
    assert!(decoded.get("metadata").is_none());
}

#[test]
fn full_toml_rejects_unrepresentable_metadata_without_changing_the_draft() {
    for metadata in [
        json!({"optional": null, "job": "review"}),
        json!({"nested": {"optional": null}}),
        json!({"values": ["review", null]}),
    ] {
        let mut input = form();
        input.metadata = metadata.to_string();
        let original = input.draft().unwrap();
        let before = original.clone();
        let error = to_toml(&original).unwrap_err();
        assert!(error.contains("Metadata contains values TOML cannot represent"));
        assert!(error.contains("Guided setup"));
        assert_eq!(original["metadata"], metadata);
        assert_eq!(original, before);
    }
}

#[test]
fn full_toml_preserves_representable_json_metadata() {
    let mut input = form();
    let metadata =
        json!({"job":"review", "options":{"enabled":false,"attempts":0}, "files":["a", "b"]});
    input.metadata = metadata.to_string();
    let text = to_toml(&input.draft().unwrap()).unwrap();
    let restored: Value = toml::from_str(&text).unwrap();
    assert_eq!(restored["metadata"], metadata);
}
#[test]
fn guided_resources_reject_invalid_capacity() {
    let mut input = form();
    input.vcpus = "0".into();
    assert!(input.spec().is_err());
    let mut input = form();
    input.memory = "63".into();
    assert!(input.spec().is_err());
}

#[test]
fn guided_modes_make_host_privileges_explicit() {
    assert_eq!(form().spec().unwrap()["security"]["mode"], "jailed");
    let mut trusted = form();
    trusted.isolation = "trusted".into();
    assert_eq!(trusted.spec().unwrap()["security"]["mode"], "trusted");
    let mut external = form();
    external.mode = "socket".into();
    external.socket = "/run/vm.sock".into();
    assert_eq!(external.spec().unwrap()["security"]["mode"], "external");
    let mut invalid = form();
    invalid.isolation = "external".into();
    assert!(invalid.spec().is_err());
}

#[test]
fn registry_credentials_preserve_references_through_toml_editing() {
    for auth in [
        json!({"kind":"basic", "username":"robot-build", "password_secret":"registry-password"}),
        json!({"kind":"bearer", "token_secret":"registry-token"}),
    ] {
        let registry = json!({"auth":auth, "token_realm":"https://auth.example.com/token", "ca_secret":"registry-ca"});
        let mut input = form();
        input.source = "oci".into();
        input.rootfs = format!("registry.example.com/job@sha256:{}", "a".repeat(64));
        input.registry = super::registry::RegistryForm::from_value(&registry);
        let spec = input.spec().unwrap();
        assert_eq!(spec["rootfs"]["registry"], registry);
        let mut edited: Value = toml::from_str(&to_toml(&spec).unwrap()).unwrap();
        edited["memory_mib"] = json!(1024);
        assert_eq!(edited["rootfs"]["registry"], registry);
    }
}

#[test]
fn registry_modes_omit_inactive_credentials_and_allow_anonymous_custom_ca() {
    let mut registry = super::registry::RegistryForm::default();
    assert!(registry.value().unwrap().is_none());
    registry.ca_secret = "registry-ca".into();
    registry.password_secret = "stale-credential".into();
    assert_eq!(
        registry.value().unwrap().unwrap(),
        json!({"ca_secret":"registry-ca"})
    );
    registry.mode = "bearer".into();
    assert!(
        registry
            .value()
            .unwrap_err()
            .contains("Registry bearer token")
    );
    registry.token_secret = "invalid secret".into();
    assert!(registry.value().is_err());
    registry.token_secret = "token".into();
    registry.token_realm = "http://auth.example.com/token".into();
    assert!(registry.value().unwrap_err().contains("HTTPS"));
    registry.token_realm.clear();
    assert!(
        registry.value().unwrap().unwrap()["auth"]
            .get("password_secret")
            .is_none()
    );
    registry.mode = "basic".into();
    registry.username = "bad:user".into();
    assert!(registry.value().unwrap_err().contains("username"));
}

#[test]
fn guided_oci_accepts_tags_and_checks_explicit_digests() {
    for image in ["alpine:latest", "registry.example.com/job:stable", "alpine"] {
        let mut input = form();
        input.source = "oci".into();
        input.rootfs = image.into();
        assert_eq!(input.spec().unwrap()["rootfs"]["image"], image);
    }
    for image in [
        "alpine@sha256:abc".into(),
        format!("alpine@sha256:{}", "A".repeat(64)),
        "https://example.com/image".into(),
    ] {
        let mut input = form();
        input.source = "oci".into();
        input.rootfs = image;
        assert!(input.spec().is_err());
    }
}

#[test]
fn guided_kernel_requires_catalog_selection() {
    for kernel in ["", "/outside/vmlinux", "https://example.com/vmlinux"] {
        let mut input = form();
        input.kernel = kernel.into();
        assert!(input.spec().is_err());
    }
}

#[test]
fn guided_edits_preserve_hidden_settings_and_remove_cleared_fields() {
    let mut base = form().spec().unwrap();
    base["attachments"] = json!([{"asset_id":"00000000-0000-0000-0000-000000000001", "destination":"/workspace/config", "mode":420}]);
    base["files"] = json!([{"path":"input", "content":"keep"}]);
    base["environment"] = json!({"TOKEN":{"kind":"secret","secret":"api-key"}});
    base["security"]["pids_max"] = json!(123);
    base["drives"] = json!([{"id":"data","asset":{"kind":"local","path":"/assets/data.ext4"},"read_only":false}]);
    base["rootfs"] = json!({"kind":"oci", "image":"alpine:3.22", "size_mib":4096,"registry":{"ca_secret":"registry-ca"}});
    base["userdata"] = json!("old script");
    base["network"] = json!({"network":"old"});
    let mut input = form();
    input.source = "oci".into();
    input.rootfs = "alpine:3.22".into();
    input.registry = super::registry::RegistryForm::from_value(&base["rootfs"]["registry"]);
    input.memory = "1024".into();
    input.rootfs_size = "4096".into();
    let updated = super::spec::merge_guided(&base, input.spec().unwrap());
    for key in [
        "attachments",
        "files",
        "environment",
        "drives",
        "security",
        "rootfs",
    ] {
        assert_eq!(updated[key], base[key], "lost {key}");
    }
    assert_eq!(updated["memory_mib"], 1024);
    assert!(updated.get("network").is_none());
    assert!(updated.get("userdata").is_none());
}
#[test]
fn toml_draft_preserves_incomplete_guided_inputs() {
    let mut input = form();
    input.kernel.clear();
    input.rootfs.clear();
    input.memory = "invalid".into();
    let draft = input.draft().unwrap();
    assert_eq!(draft["memory_mib"], "invalid");
    assert_eq!(draft["kernel"]["name"], "");
    assert!(to_toml(&draft).is_ok());
}

#[test]
fn guided_external_edits_preserve_boot_sources() {
    let mut base = form().spec().unwrap();
    base["socket"] = json!("/run/external.sock");
    base["security"]["mode"] = json!("external");
    let mut input = form();
    input.mode = "socket".into();
    input.socket = "/run/external.sock".into();
    let updated = super::spec::merge_guided(&base, input.spec().unwrap());
    assert_eq!(updated["kernel"], base["kernel"]);
    assert_eq!(updated["rootfs"], base["rootfs"]);
}

#[test]
fn complete_guided_definition_preserves_all_sections_through_toml() {
    let mut input = form();
    input.source = "oci".into();
    input.rootfs = "alpine:latest".into();
    input.rootfs_size = "8192".into();
    input.workload_mode = "keep-alive".into();
    input.command = r#"["/bin/sh", "-lc", "printf '%s\\n' 'hello world'"]"#.into();
    input.terminal = true;
    input.network = "isolated".into();
    input.address = "10.77.0.2".into();
    input.metadata = r#"{"job":"review"}"#.into();
    input.userdata = "echo setup".into();
    let base = json!({
        "environment":{"MODEL_KEY":{"secret":"api-key"}},
        "files":[{"path":"instructions","content":"review this","encoding":"utf8","mode":420}],
        "attachments":[{"asset_id":"00000000-0000-0000-0000-000000000001","destination":"/workspace/source.tar","mode":420}],
        "secret_attachments":[{"secret":"config","destination":"/etc/reviewer/config","mode":384}],
        "initrd":{"kind":"local","path":"/assets/initrd"},
        "drives":[{"id":"data","asset":{"kind":"local","path":"/assets/data"},"read_only":true}],
        "security":{"mode":"jailed","pids_max":200}
    });
    let value = super::spec::merge_guided(&base, input.spec().unwrap());
    let validated = super::draft::validate(value, &Value::Null).unwrap();
    let restored: firemage_wire::VmSpec = toml::from_str(&to_toml(&validated).unwrap()).unwrap();
    assert!(restored.terminal);
    assert_eq!(restored.metadata.unwrap()["job"], "review");
    assert_eq!(
        restored.workload.unwrap().command.unwrap()[2],
        "printf '%s\\n' 'hello world'"
    );
    assert_eq!(validated["rootfs"]["size_mib"], 8192);
    assert_eq!(validated["environment"], base["environment"]);
    assert_eq!(validated["files"][0]["content"], "review this");
    assert_eq!(
        validated["attachments"][0]["destination"],
        "/workspace/source.tar"
    );
    assert_eq!(validated["secret_attachments"][0]["secret"], "config");
    assert_eq!(validated["initrd"]["path"], "/assets/initrd");
    assert_eq!(validated["drives"][0]["read_only"], true);
    assert_eq!(restored.security.pids_max, 200);
}

#[test]
fn workload_and_metadata_validate_before_saving_and_mode_changes_clear_workload() {
    for command in ["not JSON", "{}", "[]", "[1]", r#"[""]"#] {
        let mut input = form();
        input.source = "oci".into();
        input.rootfs = "alpine".into();
        input.command = command.into();
        assert!(input.spec().is_err(), "accepted {command}");
    }
    let mut input = form();
    input.metadata = "invalid JSON".into();
    assert!(input.spec().is_err());
    let base = json!({"workload":{"mode":"keep-alive","command":["sh"]}});
    let changed = super::spec::merge_guided(&base, form().spec().unwrap());
    assert!(changed.get("workload").is_none());
}

#[test]
fn edit_validation_preserves_immutable_runtime_and_isolation() {
    let original = super::draft::validate(form().spec().unwrap(), &Value::Null).unwrap();
    let mut changed = original.clone();
    changed["security"]["mode"] = json!("trusted");
    assert!(
        super::draft::validate(changed, &original)
            .unwrap_err()
            .contains("fixed at creation")
    );
    let mut changed = original.clone();
    changed["memory_mib"] = json!(1024);
    super::draft::validate(changed, &original).unwrap();
}

#[test]
fn web_terminal_defaults_validates_and_survives_guided_toml() {
    let mut input = form();
    input.web_terminal = true;
    let value = input.spec().unwrap();
    let restored: Value = toml::from_str(&to_toml(&value).unwrap()).unwrap();
    assert_eq!(
        restored["web_terminal"]["command"],
        json!(["/bin/sh", "-i"])
    );
    let disabled = super::spec::merge_guided(&value, form().spec().unwrap());
    assert!(disabled.get("web_terminal").is_none());
    for command in ["[]", "[1]", "[\"sh\"]", "invalid"] {
        let mut input = form();
        input.web_terminal = true;
        input.shell_command = command.into();
        assert!(input.spec().is_err(), "accepted {command}");
    }
    let mut external = form();
    external.mode = "socket".into();
    external.web_terminal = true;
    assert!(external.spec().is_err());
}
