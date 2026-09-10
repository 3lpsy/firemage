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
