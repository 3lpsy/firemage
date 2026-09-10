use firemage_wire::{NetworkAttachment, NetworkPolicy, NetworkSpec};

pub fn rules(
    tap: &str,
    table: &str,
    spec: &NetworkSpec,
    attachment: &NetworkAttachment,
) -> anyhow::Result<String> {
    rules_with_ports(tap, table, spec, attachment, &[])
}
pub fn rules_with_ports(
    tap: &str,
    table: &str,
    spec: &NetworkSpec,
    attachment: &NetworkAttachment,
    ports: &[u16],
) -> anyhow::Result<String> {
    anyhow::ensure!(ports.iter().all(|p| *p >= 1024), "invalid egress port");
    let port_set = ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    spec.validate()?;
    anyhow::ensure!(
        !tap.is_empty()
            && tap.len() <= 15
            && !table.is_empty()
            && table.len() <= 32
            && tap.bytes().all(|b| b.is_ascii_alphanumeric())
            && table.bytes().all(|b| b.is_ascii_alphanumeric()),
        "invalid generated network identifiers"
    );
    anyhow::ensure!(
        spec.subnet.contains(&attachment.address)
            && attachment.address != spec.gateway
            && attachment.address != spec.subnet.network()
            && attachment.address != spec.subnet.broadcast(),
        "VM address must be usable in subnet and differ from gateway"
    );
    let parts: Vec<_> = attachment.mac.split(':').collect();
    anyhow::ensure!(
        parts.len() == 6
            && parts
                .iter()
                .all(|p| p.len() == 2 && u8::from_str_radix(p, 16).is_ok()),
        "invalid MAC address"
    );
    // Filter at TAP ingress, before host input, forwarding or bridge decisions.
    let allowed = match spec.policy {
        NetworkPolicy::Isolated => String::new(),
        NetworkPolicy::FiremageOnly => {
            if ports.is_empty() {
                String::new()
            } else {
                format!(
                    "ip saddr {} ip daddr {} tcp dport {{ {port_set} }} accept;",
                    attachment.address, spec.gateway
                )
            }
        }
        NetworkPolicy::HostOnly { address } => {
            format!("ip saddr {} ip daddr {address} accept;", attachment.address)
        }
        NetworkPolicy::Unrestricted => format!("ip saddr {} accept;", attachment.address),
    };
    let arp = if matches!(spec.policy, NetworkPolicy::Isolated) {
        String::new()
    } else {
        format!(
            "ether type arp arp saddr ip {} arp daddr ip {} accept;",
            attachment.address, spec.gateway
        )
    };
    let inbound = match spec.policy {
        NetworkPolicy::Isolated => String::new(),
        NetworkPolicy::FiremageOnly => {
            let tcp = if ports.is_empty() {
                String::new()
            } else {
                format!(
                    "ip saddr {} ip daddr {} tcp sport {{ {port_set} }} accept;",
                    spec.gateway, attachment.address
                )
            };
            format!(
                "ether type arp arp saddr ip {} arp daddr ip {} accept; {tcp}",
                spec.gateway, attachment.address
            )
        }
        NetworkPolicy::HostOnly { address } => format!(
            "ether type arp arp saddr ip {} arp daddr ip {} accept; ip saddr {address} ip daddr {} accept;",
            spec.gateway, attachment.address, attachment.address
        ),
        NetworkPolicy::Unrestricted => format!(
            "ether type arp arp saddr ip {} arp daddr ip {} accept; ip daddr {} accept;",
            spec.gateway, attachment.address, attachment.address
        ),
    };
    Ok(format!(
        r#"table netdev {table} {{
    chain ingress {{
        type filter hook ingress device "{tap}" priority -500; policy drop;
        ether saddr != {} drop;
        {arp} {allowed}
    }}
    chain egress {{
        type filter hook egress device "{tap}" priority -500; policy drop;
        {inbound}
    }}
}}
"#,
        attachment.mac
    ))
}
