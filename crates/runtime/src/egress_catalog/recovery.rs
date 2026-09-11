use crate::Runtime;
use anyhow::Context;
use firemage_wire::{EgressPolicy, NetworkSpec, VmSpec};

impl Runtime {
    pub(super) fn is_egress_recovery_pending(&self, row: &firemage_orm::vms::Model) -> bool {
        self.egress_pending(&row.id).exists() || row.error.as_deref().is_some_and(is_egress_error)
    }

    pub async fn recover_egress(&self) -> anyhow::Result<()> {
        for row in firemage_queries::vms(&self.db, None).await? {
            if matches!(row.state.as_str(), "defined" | "stopped" | "failed") {
                let _ = std::fs::remove_file(self.egress_pending(&row.id));
                continue;
            }
            let pending = self.is_egress_recovery_pending(&row);
            let row = self.refresh(row).await?;
            if row.state == "stopped" {
                continue;
            }
            let recovered = async {
                let spec: VmSpec = serde_json::from_str(&row.spec)?;
                if spec.egress.is_none() && spec.egress_policy.is_none() && !pending {
                    return Ok(());
                }
                anyhow::ensure!(
                    matches!(row.state.as_str(), "running" | "paused" | "ready"),
                    "cannot recover egress for an unverified VM process"
                );
                let attachment = spec
                    .network
                    .as_ref()
                    .context("missing recovered egress network")?;
                let network: NetworkSpec = serde_json::from_str(
                    &firemage_queries::network(&self.db, &row.owner_id, &attachment.network)
                        .await?
                        .spec,
                )?;
                if pending {
                    firemage_network::quiesce(&row.id).await?;
                }
                self.register_egress_routes(&row, &spec, &network).await?;
                if pending {
                    let policy = self.effective_egress(&row.owner_id, &spec).await?;
                    firemage_network::replace_rules(&[firemage_network::RulesReplacement {
                        id: row.id.clone(),
                        network: network.clone(),
                        attachment: attachment.clone(),
                        ports: policy.as_ref().map(EgressPolicy::ports).unwrap_or_default(),
                    }])
                    .await?;
                    firemage_network::resume(&row.id, &network, attachment).await?;
                    if let Err(error) = std::fs::remove_file(self.egress_pending(&row.id))
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        return Err(error.into());
                    }
                    if row.error.as_deref().is_some_and(is_egress_error) {
                        firemage_queries::set_vm_state(
                            &self.db,
                            row.clone(),
                            &row.state,
                            None,
                            row.pid,
                        )
                        .await?;
                    }
                }
                anyhow::Ok(())
            }
            .await;
            if let Err(error) = recovered {
                let marker = self.mark_egress_pending(&row.id);
                self.unregister_egress(&row.id).await;
                let blocked = firemage_network::suspend(&row.id).await.is_ok();
                let mut message = if blocked {
                    format!(
                        "VM egress recovery failed; its network interface is disabled: {error}. Repair its configuration and retry or restart the VM."
                    )
                } else {
                    format!(
                        "VM egress recovery failed and its network interface could not be disabled: {error}. Inspect host networking before restarting the VM."
                    )
                };
                if let Err(error) = marker {
                    message.push_str(&format!(
                        " Could not persist egress recovery marker: {error}"
                    ));
                }
                firemage_queries::set_vm_state(
                    &self.db,
                    row.clone(),
                    &row.state,
                    Some(message),
                    row.pid,
                )
                .await?;
            }
        }
        Ok(())
    }
}

pub(super) fn is_egress_error(message: &str) -> bool {
    message.starts_with("Egress update failed") || message.starts_with("VM egress recovery failed")
}
