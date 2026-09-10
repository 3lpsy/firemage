use crate::Runtime;
use firemage_wire::{NetworkAttachment, VmSpec};

impl Runtime {
    pub(crate) async fn network_tap(
        &self,
        row: &firemage_orm::vms::Model,
        net: &NetworkAttachment,
    ) -> anyhow::Result<String> {
        let _guard = self.lock("networks").await;
        let definition = firemage_queries::network(&self.db, &row.owner_id, &net.network).await?;
        for other in firemage_queries::vms(&self.db, None).await? {
            if other.id != row.id
                && !matches!(other.state.as_str(), "defined" | "stopped" | "failed")
            {
                let other: VmSpec = serde_json::from_str(&other.spec)?;
                anyhow::ensure!(
                    !other.network.is_some_and(|n| n.address == net.address),
                    "VM IP address already in use"
                );
            }
        }
        let network: firemage_wire::NetworkSpec = serde_json::from_str(&definition.spec)?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        let ports = self
            .effective_egress(&spec)
            .map(|p| p.ports())
            .unwrap_or_default();
        let tap = firemage_network::create_with_ports(&row.id, &network, net, &ports).await?;
        if let Err(error) = self.register_egress(row, &spec, &network).await {
            let _ = firemage_network::remove(&row.id).await;
            return Err(error);
        }
        Ok(tap)
    }
}
