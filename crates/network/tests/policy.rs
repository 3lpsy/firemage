use firemage_wire::{NetworkAttachment, NetworkPolicy, NetworkSpec};

fn network(policy: NetworkPolicy) -> (NetworkSpec, NetworkAttachment) {
    (
        NetworkSpec {
            name: "jobs".into(),
            subnet: "172.30.0.0/24".parse().unwrap(),
            gateway: "172.30.0.1".parse().unwrap(),
            policy,
        },
        NetworkAttachment {
            network: "jobs".into(),
            address: "172.30.0.2".parse().unwrap(),
            mac: "02:00:00:00:00:02".into(),
        },
    )
}

#[test]
fn isolated_and_host_only_rules_drop_unapproved_ingress() {
    let (mut spec, attachment) = network(NetworkPolicy::Isolated);
    let isolated =
        firemage_network::rules("fm0123456789", "fm0123456789", &spec, &attachment).unwrap();
    assert!(isolated.contains("hook ingress device \"fm0123456789\""));
    assert!(isolated.contains("policy drop;"));
    assert!(!isolated.contains("accept"));
    spec.policy = NetworkPolicy::HostOnly {
        address: "192.0.2.10".parse().unwrap(),
    };
    let host = firemage_network::rules("fm0123456789", "fm0123456789", &spec, &attachment).unwrap();
    assert!(host.contains("policy drop;"));
    assert!(host.contains("ip saddr 172.30.0.2 ip daddr 192.0.2.10 accept;"));
    assert!(host.contains("arp saddr ip 172.30.0.2 arp daddr ip 172.30.0.1 accept;"));
    assert!(host.contains("hook egress device \"fm0123456789\""));
    assert!(host.contains("ip saddr 192.0.2.10 ip daddr 172.30.0.2 accept;"));
}

#[test]
fn policy_rejects_unusable_addresses_and_rule_injection() {
    let (mut spec, mut attachment) = network(NetworkPolicy::Isolated);
    for address in ["172.30.0.0", "172.30.0.1", "172.30.0.255", "172.31.0.2"] {
        attachment.address = address.parse().unwrap();
        assert!(
            firemage_network::rules("fm0123456789", "fm0123456789", &spec, &attachment).is_err()
        );
    }
    attachment.address = "172.30.0.2".parse().unwrap();
    for identifier in ["tap; accept", "tap\"", "tap\n}"] {
        assert!(firemage_network::rules(identifier, "safe", &spec, &attachment).is_err());
        assert!(firemage_network::rules("safe", identifier, &spec, &attachment).is_err());
    }
    for gateway in ["172.30.0.0", "172.30.0.255", "172.31.0.1"] {
        spec.gateway = gateway.parse().unwrap();
        assert!(spec.validate().is_err());
    }
    spec.gateway = "172.30.0.1".parse().unwrap();
    spec.subnet = "172.30.0.0/31".parse().unwrap();
    assert!(spec.validate().is_err());
}

#[test]
fn firemage_only_permits_only_registered_tcp_ports_on_gateway() {
    let (spec, attachment) = network(NetworkPolicy::FiremageOnly);
    let rules = firemage_network::rules_with_ports(
        "fm0123456789",
        "fm0123456789",
        &spec,
        &attachment,
        &[3128, 5432],
    )
    .unwrap();
    assert!(
        rules.contains("ip saddr 172.30.0.2 ip daddr 172.30.0.1 tcp dport { 3128, 5432 } accept;")
    );
    assert!(
        rules.contains("ip saddr 172.30.0.1 ip daddr 172.30.0.2 tcp sport { 3128, 5432 } accept;")
    );
    assert!(!rules.contains("udp"));
    assert!(!rules.contains("ip6"));
    let denied =
        firemage_network::rules("fm0123456789", "fm0123456789", &spec, &attachment).unwrap();
    assert!(!denied.contains("tcp"));
    assert_eq!(denied.matches("policy drop").count(), 2);
}

#[test]
fn tap_mac_is_stable_local_unicast_and_vm_specific() {
    let mac = firemage_network::tap_mac("01234567-89ab-cdef-0123-456789abcdef").unwrap();
    assert_eq!(mac, "02:01:23:45:67:89");
    let first = u8::from_str_radix(mac.split(':').next().unwrap(), 16).unwrap();
    assert_eq!(first & 3, 2);
    assert_eq!(
        mac,
        firemage_network::tap_mac("01234567-89AB-CDEF-0123-456789ABCDEF").unwrap()
    );
    assert_ne!(
        mac,
        firemage_network::tap_mac("11234567-89ab-cdef-0123-456789abcdef").unwrap()
    );
    assert!(firemage_network::tap_mac("invalid").is_err());
}
