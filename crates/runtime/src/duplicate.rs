use crate::Runtime;
use firemage_wire::{Vm, VmSpec};

impl Runtime {
    pub async fn duplicate_vm(&self, owner: &str, id: &str, name: &str) -> anyhow::Result<Vm> {
        firemage_wire::ensure_name(name)?;
        let _source = self.lock(id).await;
        let row = firemage_queries::vm(&self.db, owner, id).await?;
        let mut spec: VmSpec = serde_json::from_str(&row.spec)?;
        anyhow::ensure!(
            spec.socket.is_none(),
            "external socket VMs cannot be duplicated; create a managed definition instead"
        );
        spec.name = name.into();
        if let Some(network) = &spec.network {
            spec.network = Some(
                self.suggest_network_address(owner, &network.network, None)
                    .await?,
            );
        }
        self.define(owner, spec).await
    }
}
