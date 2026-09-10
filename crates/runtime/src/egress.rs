use crate::Runtime;
use anyhow::Context;
use firemage_wire::{EgressPolicy, EnvironmentValue, NetworkPolicy, NetworkSpec, VmSpec};
use std::sync::Arc;

impl Runtime {
    pub async fn secrets(&self) -> anyhow::Result<&firemage_secrets::Vault> {
        self.secrets
            .get_or_try_init(|| async {
                firemage_secrets::Vault::new(self.db.clone(), &self.config.data_dir()).await
            })
            .await
    }
    pub async fn egress(&self) -> anyhow::Result<&firemage_egress_proxy::EgressManager> {
        self.egress
            .get_or_try_init(|| {
                firemage_egress_proxy::EgressManager::new(self.config.data_dir().join("egress"))
            })
            .await
    }
    pub fn effective_egress(&self, spec: &VmSpec) -> Option<EgressPolicy> {
        spec.egress.clone().map(|mut policy| {
            if policy.inherit_upstream && policy.upstream.is_none() {
                policy.upstream = self.config.egress_upstream.clone();
            }
            policy
        })
    }
    pub(crate) async fn is_restricted_network(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<bool> {
        let Some(attachment) = &spec.network else {
            return Ok(false);
        };
        let row = firemage_queries::network(&self.db, owner, &attachment.network).await?;
        let network: NetworkSpec = serde_json::from_str(&row.spec)?;
        Ok(matches!(network.policy, NetworkPolicy::FiremageOnly))
    }
    pub async fn ensure_raw_request(
        &self,
        row: &firemage_orm::vms::Model,
        input: &firemage_wire::RawRequest,
    ) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if input.method != "GET" && self.is_restricted_network(&row.owner_id, &spec).await? {
            anyhow::ensure!(
                !["/network-interfaces", "/vsock", "/snapshot/load"]
                    .iter()
                    .any(|path| input.path == *path || input.path.starts_with(&format!("{path}/"))),
                "Firemage-only networking requires managed network configuration; raw NIC, vsock and snapshot loading are unavailable"
            );
        }
        Ok(())
    }
    pub(crate) async fn validate_dependencies(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<()> {
        if self.is_restricted_network(owner, spec).await? {
            anyhow::ensure!(
                spec.socket.is_none(),
                "Firemage-only networking requires a managed VM"
            );
        }
        if let Some(policy) = self.effective_egress(spec) {
            policy.validate()?;
            let net = spec
                .network
                .as_ref()
                .context("egress requires a Firemage-only network")?;
            let row = firemage_queries::network(&self.db, owner, &net.network).await?;
            let net: NetworkSpec = serde_json::from_str(&row.spec)?;
            anyhow::ensure!(
                matches!(net.policy, NetworkPolicy::FiremageOnly),
                "egress requires a Firemage-only network"
            );
            for name in policy.secret_names() {
                self.secrets()
                    .await?
                    .resolve(owner, name)
                    .await
                    .context("referenced egress secret is unavailable")?;
            }
        }
        for value in spec.environment.values() {
            if let EnvironmentValue::Secret { secret } = value {
                self.secrets()
                    .await?
                    .resolve(owner, secret)
                    .await
                    .context("referenced environment secret is unavailable")?;
            }
        }
        Ok(())
    }
    pub(crate) async fn register_egress(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
        network: &NetworkSpec,
    ) -> anyhow::Result<()> {
        self.validate_dependencies(&row.owner_id, spec).await?;
        self.register_egress_routes(row, spec, network).await
    }
    async fn register_egress_routes(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
        network: &NetworkSpec,
    ) -> anyhow::Result<()> {
        if let Some(policy) = self.effective_egress(spec) {
            anyhow::ensure!(
                matches!(network.policy, NetworkPolicy::FiremageOnly),
                "egress requires a Firemage-only network"
            );
            let net = spec.network.as_ref().context("missing egress network")?;
            self.egress()
                .await?
                .register(
                    &row.id,
                    net.address,
                    network.gateway,
                    policy,
                    Arc::new(self.secrets().await?.scoped(row.owner_id.clone())),
                )
                .await?;
        }
        Ok(())
    }
    pub(crate) async fn unregister_egress(&self, id: &str) {
        if let Some(manager) = self.egress.get() {
            manager.unregister(id).await;
        }
    }
    pub async fn is_egress_active(&self, id: &str) -> bool {
        if let Some(manager) = self.egress.get() {
            manager.is_registered(id).await
        } else {
            false
        }
    }
    pub async fn recover_egress(&self) -> anyhow::Result<()> {
        for row in firemage_queries::vms(&self.db, None).await? {
            if matches!(row.state.as_str(), "defined" | "stopped" | "failed") {
                continue;
            }
            let row = self.refresh(row).await?;
            if row.state == "stopped" {
                continue;
            }
            let recovered = async {
                let spec: VmSpec = serde_json::from_str(&row.spec)?;
                if spec.egress.is_some() {
                    let attachment = spec
                        .network
                        .as_ref()
                        .context("missing recovered egress network")?;
                    let network =
                        firemage_queries::network(&self.db, &row.owner_id, &attachment.network)
                            .await?;
                    self.register_egress_routes(&row, &spec, &serde_json::from_str(&network.spec)?)
                        .await?;
                }
                anyhow::Ok(())
            }
            .await;
            if recovered.is_err() {
                self.unregister_egress(&row.id).await;
                let state = row.state.clone();
                let pid = row.pid;
                let message = if firemage_network::suspend(&row.id).await.is_ok() {
                    "VM egress recovery failed; its network interface is disabled. Repair its configuration and restart the VM."
                } else {
                    "VM egress recovery failed and its network interface could not be disabled. Inspect host networking before restarting the VM."
                };
                firemage_queries::set_vm_state(&self.db, row, &state, Some(message.into()), pid)
                    .await?;
            }
        }
        Ok(())
    }
}
