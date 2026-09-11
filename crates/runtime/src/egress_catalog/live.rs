use crate::Runtime;
use anyhow::Context;
use firemage_wire::{EgressPolicy, NetworkSpec, VmSpec};
use std::{future::Future, sync::Arc};

pub(super) struct Change {
    pub row: firemage_orm::vms::Model,
    pub spec: VmSpec,
    pub network: NetworkSpec,
    pub before: Option<EgressPolicy>,
    pub after: Option<EgressPolicy>,
}
impl Runtime {
    pub(super) async fn lock_egress_vms(
        &self,
        rows: &[firemage_orm::vms::Model],
    ) -> Vec<tokio::sync::OwnedMutexGuard<()>> {
        let mut ids: Vec<_> = rows.iter().map(|row| &row.id).collect();
        ids.sort();
        ids.dedup();
        let mut locks = Vec::new();
        for id in ids {
            locks.push(self.lock(id).await);
        }
        locks
    }
    pub(super) async fn live_change(
        &self,
        row: firemage_orm::vms::Model,
        mut after: Option<EgressPolicy>,
    ) -> anyhow::Result<Option<Change>> {
        // Lifecycle actions can finish while a shared update waits for VM locks.
        let row = firemage_queries::vm(&self.db, &row.owner_id, &row.id).await?;
        let row = self.refresh(row).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if let Some(http) = after.as_mut().and_then(|policy| policy.http.as_mut()) {
            http.port = spec.egress_http_port;
        }
        if let Some(policy) = &after {
            policy.validate()?;
        }
        if matches!(row.state.as_str(), "defined" | "stopped" | "failed") {
            return Ok(None);
        }
        anyhow::ensure!(
            matches!(row.state.as_str(), "running" | "paused" | "ready"),
            "VM {} has unknown runtime state; repair it before changing shared egress",
            row.name
        );
        let attachment = spec.network.as_ref().context("missing egress network")?;
        let network: NetworkSpec = serde_json::from_str(
            &firemage_queries::network(&self.db, &row.owner_id, &attachment.network)
                .await?
                .spec,
        )?;
        anyhow::ensure!(
            matches!(network.policy, firemage_wire::NetworkPolicy::FiremageOnly),
            "egress requires a Firemage-only network"
        );
        // If old credentials cannot resolve, rollback must leave access blocked.
        let before = self
            .effective_egress(&row.owner_id, &spec)
            .await
            .unwrap_or(None);
        let adding_http = after.as_ref().is_some_and(|p| p.http.is_some())
            && before.as_ref().is_none_or(|p| p.http.is_none());
        anyhow::ensure!(
            !adding_http || self.directory(&row.id).join("egress-bootstrap").exists(),
            "VM {} needs stop and prepare once before enabling HTTP egress; its current guest has no proxy bootstrap",
            row.name
        );
        Ok(Some(Change {
            row,
            spec,
            network,
            before,
            after,
        }))
    }
    pub(crate) fn egress_pending(&self, id: &str) -> std::path::PathBuf {
        self.config.data_dir().join("egress-pending").join(id)
    }
    pub(super) fn mark_egress_pending(&self, id: &str) -> anyhow::Result<()> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        firemage_wire::ensure_asset_id(id)?;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(self.config.data_dir().join("egress-pending"))?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(self.egress_pending(id))?;
        file.sync_all()?;
        Ok(())
    }
    pub(super) async fn replace_egress_engine(
        &self,
        changes: &[Change],
        after: bool,
    ) -> anyhow::Result<()> {
        let mut replacements = Vec::new();
        let mut rules = Vec::new();
        for change in changes {
            let policy = if after { &change.after } else { &change.before };
            let attachment = change
                .spec
                .network
                .as_ref()
                .context("missing network attachment")?;
            replacements.push(firemage_egress_proxy::Replacement {
                id: change.row.id.clone(),
                guest: attachment.address,
                gateway: change.network.gateway,
                policy: policy.clone(),
                secrets: Arc::new(self.secrets().await?.scoped(change.row.owner_id.clone())),
            });
            rules.push(firemage_network::RulesReplacement {
                id: change.row.id.clone(),
                network: change.network.clone(),
                attachment: attachment.clone(),
                ports: policy.as_ref().map(EgressPolicy::ports).unwrap_or_default(),
            });
        }
        self.egress().await?.replace_many(replacements).await?;
        firemage_network::replace_rules(&rules).await
    }
    async fn resume_egress_changes(&self, changes: &[Change]) -> anyhow::Result<()> {
        for change in changes {
            let attachment = change
                .spec
                .network
                .as_ref()
                .context("missing network attachment")?;
            firemage_network::resume(&change.row.id, &change.network, attachment).await?;
        }
        for change in changes {
            let _ = std::fs::remove_file(self.egress_pending(&change.row.id));
            if change
                .row
                .error
                .as_deref()
                .is_some_and(super::recovery::is_egress_error)
            {
                firemage_queries::set_vm_state(
                    &self.db,
                    change.row.clone(),
                    &change.row.state,
                    None,
                    change.row.pid,
                )
                .await?;
            }
        }
        Ok(())
    }
    async fn block_failed_egress(&self, changes: &[Change], error: &anyhow::Error) {
        for change in changes {
            let _ = self.mark_egress_pending(&change.row.id);
            let _ = firemage_network::suspend(&change.row.id).await;
            self.unregister_egress(&change.row.id).await;
            let _ = firemage_queries::set_vm_state(&self.db, change.row.clone(), &change.row.state, Some(format!("Egress update failed: {error}; retry the policy update or restart Firemage to recover network access")), change.row.pid).await;
        }
    }
    pub(super) async fn apply_egress_changes<T, F>(
        &self,
        changes: Vec<Change>,
        commit: F,
    ) -> anyhow::Result<T>
    where
        F: Future<Output = anyhow::Result<T>>,
    {
        if changes.is_empty() {
            return commit.await;
        }
        for change in &changes {
            self.mark_egress_pending(&change.row.id)?;
        }
        let staged = async {
            for change in &changes {
                firemage_network::quiesce(&change.row.id).await?;
            }
            self.replace_egress_engine(&changes, true).await?;
            commit.await
        }
        .await;
        match staged {
            Ok(result) => {
                if let Err(error) = self.resume_egress_changes(&changes).await {
                    self.block_failed_egress(&changes, &error).await;
                    anyhow::bail!("policy saved, but live network activation failed: {error}");
                }
                Ok(result)
            }
            Err(error) => {
                let restored = async {
                    for change in &changes {
                        firemage_network::quiesce(&change.row.id).await?;
                    }
                    self.replace_egress_engine(&changes, false).await?;
                    self.resume_egress_changes(&changes).await
                }
                .await;
                if let Err(rollback) = restored {
                    self.block_failed_egress(&changes, &rollback).await;
                }
                Err(error)
            }
        }
    }
}
