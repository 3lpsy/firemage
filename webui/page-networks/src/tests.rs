use crate::gateway::Gateway;

#[test]
fn new_network_gateway_follows_valid_subnet_changes() {
    let mut gateway = Gateway::new(None, "172.30.0.0/24");
    assert_eq!(gateway.value, "172.30.0.1");
    gateway.update_subnet("192.0.2.129/25");
    assert_eq!(gateway.value, "192.0.2.129");
    for invalid in ["192.0.2.", "192.0.2.0/31", "192.0.2.0/32", "::/64"] {
        gateway.update_subnet(invalid);
        assert_eq!(gateway.value, "192.0.2.129");
    }
    gateway.update_subnet("198.51.100.252/30");
    assert_eq!(gateway.value, "198.51.100.253");
}

#[test]
fn existing_and_manually_entered_gateways_are_preserved() {
    let mut existing = Gateway::new(Some("192.0.2.254"), "192.0.2.0/24");
    existing.update_subnet("192.0.2.0/23");
    assert_eq!(existing.value, "192.0.2.254");
    let mut new = Gateway::new(None, "192.0.2.0/24");
    new.edit("192.0.2.200".into());
    new.update_subnet("192.0.2.0/23");
    assert_eq!(new.value, "192.0.2.200");
    new.edit(String::new());
    new.update_subnet("198.51.100.0/24");
    assert!(new.value.is_empty());
}
