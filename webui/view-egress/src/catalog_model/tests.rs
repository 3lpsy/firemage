use super::*;
#[test]
fn catalog_policy_uses_references_and_stable_port() {
    let input = policy_input(" review ", json!({"http":{"port":9999,"rules":[]},"tunnels":[],"upstream":{"url":"http://old.example:3128"}}), "proxy-id", "").unwrap();
    assert_eq!(input["alias"], "review");
    assert_eq!(input["policy"]["http"]["port"], 3128);
    assert!(input["policy"].get("upstream").is_none());
    assert_eq!(input["upstream_proxy_id"], "proxy-id");
    assert_eq!(input["policy"]["inherit_upstream"], false);
    let direct = policy_input("deny-all", json!({"tunnels":[]}), "", "").unwrap();
    assert!(direct["upstream_proxy_id"].is_null());
    assert_eq!(direct["policy"]["inherit_upstream"], false);
}
#[test]
fn references_include_inactive_vms() {
    let row = json!({"vms":[{"id":"a","state":"stopped"},{"id":"b","state":"running"}]});
    assert_eq!(usage(&row, "vm"), 2);
    assert_eq!(running(&row), 1);
}
