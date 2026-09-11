use firemage_wire::{EgressPolicyUpdate, UpstreamProxyInput, UpstreamProxyUpdate, VmSpec};
use serde_json::json;

#[test]
fn catalog_updates_use_flat_payloads_and_reject_unknown_fields() {
    let policy = json!({"alias":"review","policy":{"inherit_upstream":false},"upstream_proxy_id":null,"revision":1});
    let decoded: EgressPolicyUpdate = serde_json::from_value(policy.clone()).unwrap();
    assert_eq!(decoded.input.alias, "review");
    decoded.input.validate().unwrap();
    let mut unknown = policy;
    unknown["unexpected"] = true.into();
    assert!(serde_json::from_value::<EgressPolicyUpdate>(unknown).is_err());
    let proxy = json!({"alias":"upstream","proxy":{"url":"http://proxy.example:3128"},"ca_secret":null,"revision":1});
    let decoded: UpstreamProxyUpdate = serde_json::from_value(proxy.clone()).unwrap();
    decoded.input.validate().unwrap();
    let mut unknown = proxy;
    unknown["unexpected"] = true.into();
    assert!(serde_json::from_value::<UpstreamProxyUpdate>(unknown).is_err());
}

#[test]
fn catalog_vm_references_validate_ids_ports_and_inline_conflicts() {
    let mut input = json!({"name":"review","network":{"network":"review","address":"10.70.1.2","mac":"02:00:00:00:00:01"},"egress_policy":"12345678-1234-1234-1234-123456789abc"});
    let spec: VmSpec = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(spec.egress_http_port, 3128);
    spec.validate().unwrap();
    input["egress"] = json!({});
    assert!(
        serde_json::from_value::<VmSpec>(input.clone())
            .unwrap()
            .validate()
            .is_err()
    );
    input["egress"] = serde_json::Value::Null;
    input["egress_http_port"] = 80.into();
    assert!(
        serde_json::from_value::<VmSpec>(input.clone())
            .unwrap()
            .validate()
            .is_err()
    );
    input["egress_http_port"] = 3128.into();
    input["egress_policy"] = "arbitrary-path".into();
    assert!(
        serde_json::from_value::<VmSpec>(input)
            .unwrap()
            .validate()
            .is_err()
    );
}

#[test]
fn proxy_credentials_require_secret_references_and_ca_sources_are_exclusive() {
    let input = json!({"alias":"upstream","proxy":{"url":"http://proxy.example:3128","username":"review","password":"literal-key"}});
    assert!(
        serde_json::from_value::<UpstreamProxyInput>(input)
            .unwrap()
            .validate()
            .is_err()
    );
    let input = json!({"alias":"upstream","proxy":{"url":"http://proxy.example:3128","ca_pem":"-----BEGIN CERTIFICATE-----\npublic\n-----END CERTIFICATE-----"},"ca_secret":"trusted-ca"});
    assert!(
        serde_json::from_value::<UpstreamProxyInput>(input)
            .unwrap()
            .validate()
            .is_err()
    );
}
