use crate::{Asset, RegistryAccess, VmSpec};
use serde_json::json;

fn asset(registry: serde_json::Value) -> serde_json::Value {
    json!({"kind":"oci", "image":format!("registry.example/team/image@sha256:{}", "a".repeat(64)), "size_mib":2048, "registry":registry})
}

#[test]
fn registry_access_roundtrips_only_secret_references() {
    let input = asset(
        json!({"auth":{"kind":"basic", "username":"robot", "password_secret":"pull-password"}, "token_realm":"https://auth.example/token", "ca_secret":"registry-ca"}),
    );
    let spec: VmSpec = serde_json::from_value(json!({"name":"oci", "rootfs":input})).unwrap();
    spec.validate().unwrap();
    let Asset::Oci {
        registry: Some(access),
        ..
    } = spec.rootfs.as_ref().unwrap()
    else {
        panic!("OCI access missing")
    };
    assert_eq!(access.secret_names(), vec!["pull-password", "registry-ca"]);
    assert!(!access.is_anonymous());
    let stored = serde_json::to_value(spec).unwrap();
    assert_eq!(
        stored["rootfs"]["registry"]["auth"]["password_secret"],
        "pull-password"
    );
    assert!(
        stored["rootfs"]["registry"]["auth"]
            .get("password")
            .is_none()
    );
    assert!(serde_json::from_value::<Asset>(asset(json!({"auth":{"kind":"basic", "username":"robot", "password_secret":"ref", "password":"inline-value"}}))).is_err());
}

#[test]
fn registry_realm_and_secret_references_are_validated() {
    for realm in [
        "http://auth.example/token",
        "https://user:password@auth.example/token",
        "https://auth.example/token?credential=value",
        "https://auth.example/token#fragment",
        "https://auth.example/\ntoken",
    ] {
        let access: RegistryAccess = serde_json::from_value(json!({"token_realm":realm})).unwrap();
        assert!(access.validate().is_err(), "accepted unsafe realm");
    }
    for auth in [
        json!({"kind":"basic","username":"user:other","password_secret":"valid"}),
        json!({"kind":"bearer","token_secret":"../secret"}),
        json!({"kind":"basic","username":"robot","password_secret":""}),
    ] {
        let access: RegistryAccess = serde_json::from_value(json!({"auth":auth})).unwrap();
        assert!(access.validate().is_err());
    }
}

#[test]
fn oci_assets_require_digest_and_bounded_disk_before_definition() {
    for (image, size) in [
        ("alpine:latest".to_owned(), 2048),
        (format!("alpine@sha256:{}", "A".repeat(64)), 2048),
        (format!("image@sha256:{}", "a".repeat(64)), 32769),
        (
            format!("https://registry.example/image@sha256:{}", "a".repeat(64)),
            2048,
        ),
    ] {
        let spec: VmSpec = serde_json::from_value(
            json!({"name":"invalid", "rootfs":{"kind":"oci","image":image,"size_mib":size}}),
        )
        .unwrap();
        assert!(spec.validate().is_err());
    }
    let spec: VmSpec = serde_json::from_value(json!({"name":"anonymous", "rootfs":{"kind":"oci","image":format!("alpine@sha256:{}", "a".repeat(64))}})).unwrap();
    spec.validate().unwrap();
}
