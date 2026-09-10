use super::spec::{Form, to_toml};
use serde_json::{Value, json};
fn form() -> Form {
    Form {
        name: "runner".into(),
        mode: "managed".into(),
        socket: String::new(),
        kernel: "/assets/vmlinux".into(),
        kernel_sha: String::new(),
        source: "local".into(),
        rootfs: "/assets/root.ext4".into(),
        rootfs_sha: String::new(),
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
        json!({ "kind" : "local", "path" : "/assets/vmlinux" })
    );
}
#[test]
fn remote_assets_preserve_verification_and_oci_keeps_image_reference() {
    let mut input = form();
    input.kernel = "https://example.test/kernel".into();
    input.kernel_sha = "ab".repeat(32);
    input.source = "oci".into();
    input.rootfs = "example.test/runner@sha256:abc".into();
    let spec = input.spec().unwrap();
    assert_eq!(spec["kernel"]["sha256"], "ab".repeat(32));
    assert_eq!(spec["rootfs"]["kind"], "oci");
    assert_eq!(spec["rootfs"]["image"], "example.test/runner@sha256:abc");
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
