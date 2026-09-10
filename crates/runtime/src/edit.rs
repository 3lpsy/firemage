use crate::Runtime;
use firemage_wire::{Vm, VmSpec};
impl Runtime {
    pub async fn update(&self, owner: &str, id: &str, spec: VmSpec) -> anyhow::Result<Vm> {
        spec.validate()?;
        let _guard = self.lock(id).await;
        let _network_guard = self.lock("networks").await;
        self.validate_dependencies(owner, &spec).await?;
        let row = self
            .refresh(firemage_queries::vm(&self.db, owner, id).await?)
            .await?;
        anyhow::ensure!(
            matches!(row.state.as_str(), "defined" | "stopped" | "failed")
                && !self.children.lock().await.contains_key(id),
            "stop VM before editing its configuration"
        );
        let previous: VmSpec = serde_json::from_str(&row.spec)?;
        anyhow::ensure!(
            previous.socket == spec.socket,
            "attached socket cannot be changed; define a new VM"
        );
        if let Some(network) = &spec.network {
            firemage_queries::network(&self.db, owner, &network.network).await?;
        }
        if self.directory(id).join("rootfs.ext4").try_exists()? {
            anyhow::ensure!(
                serde_json::to_value(&previous.rootfs)? == serde_json::to_value(&spec.rootfs)?,
                "existing VM rootfs cannot be replaced; define a new VM to change it"
            );
        }
        let drive_ids: std::collections::HashSet<_> = previous
            .drives
            .iter()
            .chain(&spec.drives)
            .map(|drive| &drive.id)
            .collect();
        for drive_id in drive_ids {
            if self
                .directory(id)
                .join(format!("drive-{drive_id}.img"))
                .try_exists()?
            {
                let old = previous.drives.iter().find(|drive| &drive.id == drive_id);
                let new = spec.drives.iter().find(|drive| &drive.id == drive_id);
                anyhow::ensure!(
                    old.is_some()
                        && new.is_some()
                        && serde_json::to_value(old)? == serde_json::to_value(new)?,
                    "existing VM drive {drive_id} cannot be replaced or removed; define a new VM to change it"
                );
            }
        }
        crate::view(
            firemage_queries::update_vm_spec(
                &self.db,
                row,
                spec.name.clone(),
                serde_json::to_string(&spec)?,
            )
            .await?,
        )
    }
}
