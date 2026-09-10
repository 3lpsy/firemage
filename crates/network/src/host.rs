use anyhow::Context;
use firemage_wire::{NetworkAttachment, NetworkSpec};
use tokio::{io::AsyncWriteExt, process::Command};

pub fn identifiers(id: &str) -> anyhow::Result<(String, String)> {
    let short: String = id.chars().filter(|c| *c != '-').take(10).collect();
    anyhow::ensure!(
        short.len() == 10 && short.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid VM id"
    );
    Ok((format!("fm{short}"), format!("fm{short}")))
}
// Snapshot guests retain the gateway's ARP entry when the TAP is recreated.
pub fn tap_mac(id: &str) -> anyhow::Result<String> {
    let (tap, _) = identifiers(id)?;
    Ok(format!(
        "02:{}:{}:{}:{}:{}",
        &tap[2..4],
        &tap[4..6],
        &tap[6..8],
        &tap[8..10],
        &tap[10..12]
    )
    .to_ascii_lowercase())
}
async fn ip(args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("ip").args(args).output().await?;
    anyhow::ensure!(
        output.status.success(),
        "ip failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
pub async fn create(
    id: &str,
    spec: &NetworkSpec,
    attachment: &NetworkAttachment,
) -> anyhow::Result<String> {
    create_with_ports(id, spec, attachment, &[]).await
}
pub async fn create_with_ports(
    id: &str,
    spec: &NetworkSpec,
    attachment: &NetworkAttachment,
    ports: &[u16],
) -> anyhow::Result<String> {
    let (tap, table) = identifiers(id)?;
    let rules = crate::rules_with_ports(&tap, &table, spec, attachment, ports)?;
    if let firemage_wire::NetworkPolicy::HostOnly { address } = spec.policy
        && address != spec.gateway
    {
        let output = Command::new("ip")
            .args(["-j", "-4", "address", "show"])
            .output()
            .await?;
        anyhow::ensure!(output.status.success(), "cannot inspect host addresses");
        let interfaces: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let is_local = interfaces.as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["addr_info"].as_array().is_some_and(|addresses| {
                    addresses
                        .iter()
                        .any(|entry| entry["local"].as_str() == Some(&address.to_string()))
                })
            })
        });
        anyhow::ensure!(
            is_local,
            "host-only target must be an address assigned to this host"
        );
    }
    ip(&["tuntap", "add", "dev", &tap, "mode", "tap"]).await?;
    let result = async {
        let mut nft = Command::new("nft")
            .args(["-f", "-"])
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        nft.stdin
            .take()
            .context("nft stdin")?
            .write_all(rules.as_bytes())
            .await?;
        let output = nft.wait_with_output().await?;
        anyhow::ensure!(
            output.status.success(),
            "nft failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        ip(&["link", "set", &tap, "address", &tap_mac(id)?]).await?;
        ip(&["link", "set", &tap, "up"]).await?;
        ip(&["addr", "add", &format!("{}/32", spec.gateway), "dev", &tap]).await?;
        ip(&[
            "route",
            "add",
            &format!("{}/32", attachment.address),
            "dev",
            &tap,
        ])
        .await?;
        anyhow::Ok(())
    }
    .await;
    if result.is_err() {
        let _ = remove(id).await;
    }
    result?;
    Ok(tap)
}
pub async fn remove(id: &str) -> anyhow::Result<()> {
    let (tap, table) = identifiers(id)?;
    let _ = ip(&["link", "set", &tap, "down"]).await;
    let _ = Command::new("nft")
        .args(["delete", "table", "netdev", &table])
        .output()
        .await?;
    ip(&["link", "delete", &tap]).await
}

// Keep the TAP and its filters installed while disabling guest communication.
pub async fn suspend(id: &str) -> anyhow::Result<()> {
    let (tap, _) = identifiers(id)?;
    ip(&["link", "set", &tap, "down"]).await
}
