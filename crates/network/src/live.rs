use anyhow::Context;
use firemage_wire::{NetworkAttachment, NetworkSpec};
use tokio::{io::AsyncWriteExt, process::Command};

pub struct RulesReplacement {
    pub id: String,
    pub network: NetworkSpec,
    pub attachment: NetworkAttachment,
    pub ports: Vec<u16>,
}

/// Replace all affected filters in one nft transaction without changing TAP identity.
pub async fn replace_rules(updates: &[RulesReplacement]) -> anyhow::Result<()> {
    if updates.is_empty() {
        return Ok(());
    }
    let mut rules = String::new();
    let mut seen = std::collections::HashSet::new();
    for update in updates {
        let (tap, table) = crate::identifiers(&update.id)?;
        anyhow::ensure!(seen.insert(table.clone()), "duplicate VM firewall update");
        let next = crate::rules_with_ports(
            &tap,
            &table,
            &update.network,
            &update.attachment,
            &update.ports,
        )?;
        rules.push_str(&format!("delete table netdev {table}\n{next}"));
    }
    nft(&rules).await
}

async fn nft(rules: &str) -> anyhow::Result<()> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("nft stdin")?
        .write_all(rules.as_bytes())
        .await?;
    let output = child.wait_with_output().await?;
    anyhow::ensure!(
        output.status.success(),
        "nft policy update failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Block new guest traffic while keeping the TAP up so revoked sockets can send FIN/RST.
pub async fn quiesce(id: &str) -> anyhow::Result<()> {
    firemage_wire::ensure_asset_id(id)?;
    let (tap, table) = crate::identifiers(id)?;
    nft(&format!("add table netdev {table}pending\nadd chain netdev {table}pending ingress {{ type filter hook ingress device \"{tap}\" priority -501; policy drop; }}\n")).await
}

/// Restore routes after a previous hard suspension, then remove the guest traffic gate.
pub async fn resume(
    id: &str,
    network: &NetworkSpec,
    attachment: &NetworkAttachment,
) -> anyhow::Result<()> {
    let (tap, table) = crate::identifiers(id)?;
    crate::rules_with_ports(&tap, &table, network, attachment, &[])?;
    crate::host::ip(&["link", "set", &tap, "up"]).await?;
    crate::host::ip(&[
        "addr",
        "replace",
        &format!("{}/32", network.gateway),
        "dev",
        &tap,
    ])
    .await?;
    crate::host::ip(&[
        "route",
        "replace",
        &format!("{}/32", attachment.address),
        "dev",
        &tap,
    ])
    .await?;
    nft(&format!("delete table netdev {table}pending\n")).await
}
