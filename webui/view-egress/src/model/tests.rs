use super::*;
#[test]
fn policy_edits_preserve_advanced_vm_and_tls_settings() {
    let original = json!({"network":{"network":"restricted"},"files":[{"path":"input","content":"value"}],"egress":{"http":{"port":3128,"rules":[],"upstream_ca_pem":"private CA"},"tunnels":[]}});
    let mut draft = Draft::new(&original["egress"]);
    let mut rule = new_rule();
    rule["host"] = json!("api.example.com");
    rule["methods"] = json!("GET, POST");
    draft.rules.push(rule);
    let edited = draft.spec(&original).unwrap();
    assert_eq!(edited["files"], original["files"]);
    assert_eq!(edited["egress"]["http"]["upstream_ca_pem"], "private CA");
    assert_eq!(
        edited["egress"]["http"]["rules"][0]["methods"],
        json!(["GET", "POST"])
    );
}
#[test]
fn invalid_ports_and_paths_cannot_be_saved() {
    let mut draft = Draft::new(&json!({"http":{"rules":[]}}));
    draft.port = "0".into();
    assert!(draft.spec(&json!({"network":{}})).is_err());
    draft.port = "3128".into();
    let mut rule = new_rule();
    rule["host"] = json!("example.com");
    rule["path_prefix"] = json!("api");
    draft.rules.push(rule);
    assert!(draft.spec(&json!({"network":{}})).is_err());
}
#[test]
fn clearing_routes_removes_egress_without_changing_other_fields() {
    let original = json!({"name":"runner","egress":{"tunnels":[]}});
    assert_eq!(
        Draft::new(&original["egress"]).spec(&original).unwrap(),
        json!({"name":"runner"})
    );
}
