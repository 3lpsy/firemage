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
    pub async fn effective_egress(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<Option<EgressPolicy>> {
        let mut policy = if let Some(id) = &spec.egress_policy {
            let row = firemage_queries::egress_policy(&self.db, owner, id).await?;
            let mut policy = self.resolve_catalog_policy(owner, &row).await?;
            if let Some(http) = &mut policy.http {
                http.port = spec.egress_http_port;
            }
            Some(policy)
        } else {
            spec.egress.clone()
        };
        if let Some(policy) = &mut policy
            && policy.inherit_upstream
            && policy.upstream.is_none()
        {
            policy.upstream = self.config.egress_upstream.clone();
        }
        Ok(policy)
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
        anyhow::ensure!(
            input.method == "GET"
                || !(input.path == "/boot-source" || input.path.starts_with("/boot-source/")),
            "raw boot-source mutations are disabled; select a kernel through the VM configuration"
        );
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        let is_unrestricted = self.config.allow_unrestricted_raw_api == Some(true)
            && match spec.security.mode {
                firemage_wire::IsolationMode::Jailed => false,
                firemage_wire::IsolationMode::Trusted => {
                    self.config.allow_trusted_vms == Some(true)
                }
                firemage_wire::IsolationMode::External => {
                    self.config.allow_external_vms == Some(true)
                }
            };
        if input.method != "GET" && !is_unrestricted {
            anyhow::ensure!(
                matches!(
                    (input.method.as_str(), input.path.as_str()),
                    ("PUT", "/mmds" | "/actions" | "/balloon")
                        | (
                            "PATCH",
                            "/mmds" | "/vm" | "/balloon" | "/balloon/statistics"
                        )
                ),
                "raw host resource mutations require trusted/external mode and host allow_unrestricted_raw_api; jailed VMs always require managed configuration"
            );
        }
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
        self.validate_registry_dependencies(owner, spec).await?;
        self.validate_asset_attachments(owner, spec).await?;
        self.validate_secret_attachments(owner, spec).await?;
        if self.is_restricted_network(owner, spec).await? {
            anyhow::ensure!(
                spec.socket.is_none(),
                "Firemage-only networking requires a managed VM"
            );
        }
        if let Some(policy) = self.effective_egress(owner, spec).await? {
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
    pub(crate) async fn register_egress_routes(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
        network: &NetworkSpec,
    ) -> anyhow::Result<()> {
        if let Some(policy) = self.effective_egress(&row.owner_id, spec).await? {
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
}
