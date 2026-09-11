use firemage_wire::{NetworkAttachment, NetworkPolicy, NetworkSpec};
use std::{os::unix::fs::PermissionsExt, process::Command};

const CHILD: &str = "FIREMAGE_NETWORK_CONTRACT_CHILD";
const ID: &str = "01234567-89ab-cdef-0123-456789abcdef";

#[tokio::test]
async fn live_update_keeps_tap_up_and_restores_routes_before_opening_gate() {
    if std::env::var_os(CHILD).is_some() {
        let network = NetworkSpec {
            name: "jobs".into(),
            subnet: "172.30.0.0/24".parse().unwrap(),
            gateway: "172.30.0.1".parse().unwrap(),
            policy: NetworkPolicy::FiremageOnly,
        };
        let attachment = NetworkAttachment {
            network: "jobs".into(),
            address: "172.30.0.2".parse().unwrap(),
            mac: "02:00:00:00:00:02".into(),
        };
        firemage_network::quiesce(ID).await.unwrap();
        firemage_network::replace_rules(&[firemage_network::RulesReplacement {
            id: ID.into(),
            network: network.clone(),
            attachment: attachment.clone(),
            ports: vec![3128],
        }])
        .await
        .unwrap();
        let result = firemage_network::resume(ID, &network, &attachment).await;
        assert_eq!(
            result.is_err(),
            std::env::var_os("FIREMAGE_NETWORK_FAIL_ROUTE").is_some()
        );
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    for tool in ["nft", "ip"] {
        let file = directory.path().join(tool);
        std::fs::write(&file, format!("#!/bin/sh\nprintf '{tool} %s\\n' \"$*\" >> \"$FIREMAGE_NETWORK_TRACE\"\nif [ '{tool}' = nft ]; then /bin/cat >> \"$FIREMAGE_NETWORK_TRACE\"; fi\nif [ '{tool}' = ip ] && [ \"$1\" = route ] && [ -n \"$FIREMAGE_NETWORK_FAIL_ROUTE\" ]; then exit 1; fi\n")).unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    for failing in [false, true] {
        let trace = directory
            .path()
            .join(if failing { "failure" } else { "success" });
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "live_update_keeps_tap_up_and_restores_routes_before_opening_gate",
            ])
            .env(CHILD, "1")
            .env("PATH", directory.path())
            .env("FIREMAGE_NETWORK_TRACE", &trace);
        if failing {
            command.env("FIREMAGE_NETWORK_FAIL_ROUTE", "1");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let trace = std::fs::read_to_string(trace).unwrap();
        let gate = trace.find("add table netdev fm0123456789pending").unwrap();
        let replacement = trace.find("delete table netdev fm0123456789\n").unwrap();
        let route = trace
            .find("ip route replace 172.30.0.2/32 dev fm0123456789")
            .unwrap();
        assert!(gate < replacement && replacement < route);
        assert!(trace.contains("ip addr replace 172.30.0.1/32 dev fm0123456789"));
        assert!(!trace.contains(" down"));
        assert!(trace[gate..replacement].contains("hook ingress"));
        assert!(!trace[gate..replacement].contains("hook egress"));
        let opened = trace.find("delete table netdev fm0123456789pending\n");
        if failing {
            assert!(opened.is_none(), "route failure must retain the gate");
        } else {
            assert!(opened.unwrap() > route);
        }
    }
}
