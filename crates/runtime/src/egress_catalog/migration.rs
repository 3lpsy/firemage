use crate::Runtime;
use firemage_wire::{EgressPolicyInput, UpstreamProxyInput, VmSpec};

impl Runtime {
    /// Import each legacy definition separately so unrelated VMs do not become coupled.
    pub async fn migrate_egress_catalog(&self) -> anyhow::Result<()> {
        let _catalog = self.lock("egress-catalog").await;
        for row in firemage_queries::vms(&self.db, None).await? {
            let mut spec: VmSpec = serde_json::from_str(&row.spec)?;
            if spec.egress_policy.is_some() || spec.egress.is_none() {
                continue;
            }
            self.import_inline_egress(&row.owner_id, &row.id, &mut spec)
                .await?;
            firemage_queries::set_vm_egress_policy(
                &self.db,
                &row.owner_id,
                &row.id,
                spec.egress_policy.as_deref(),
                spec.egress_http_port,
            )
            .await?;
        }
        Ok(())
    }
    pub(crate) async fn normalize_egress(
        &self,
        owner: &str,
        spec: &mut VmSpec,
    ) -> anyhow::Result<()> {
        if spec.egress.is_some() {
            anyhow::ensure!(
                spec.egress_policy.is_none(),
                "choose a policy reference or inline egress, not both"
            );
            self.import_inline_egress(owner, &uuid::Uuid::new_v4().to_string(), spec)
                .await?;
        }
        Ok(())
    }
    async fn import_inline_egress(
        &self,
        owner: &str,
        key: &str,
        spec: &mut VmSpec,
    ) -> anyhow::Result<()> {
        let Some(mut policy) = spec.egress.clone() else {
            return Ok(());
        };
        policy.validate()?;
        let alias = format!("vm-{key}");
        let proxies = firemage_queries::upstream_proxies(&self.db, Some(owner)).await?;
        let upstream_proxy_id = if let Some(proxy) = policy.upstream.take() {
            if let Some(existing) = proxies.into_iter().find(|row| row.alias == alias) {
                anyhow::ensure!(
                    serde_json::from_str::<firemage_wire::UpstreamProxy>(&existing.spec)? == proxy,
                    "legacy proxy migration alias conflict"
                );
                Some(existing.id)
            } else {
                Some(
                    firemage_queries::insert_upstream_proxy(
                        &self.db,
                        owner,
                        &UpstreamProxyInput {
                            alias: alias.clone(),
                            proxy,
                            ca_secret: None,
                        },
                    )
                    .await?
                    .id,
                )
            }
        } else {
            None
        };
        spec.egress_http_port = policy.http.as_ref().map(|http| http.port).unwrap_or(3128);
        let policies = firemage_queries::egress_policies(&self.db, Some(owner)).await?;
        let id = if let Some(existing) = policies.into_iter().find(|row| row.alias == alias) {
            anyhow::ensure!(
                serde_json::from_str::<firemage_wire::EgressPolicy>(&existing.spec)? == policy
                    && existing.upstream_proxy_id == upstream_proxy_id,
                "legacy policy migration alias conflict"
            );
            existing.id
        } else {
            firemage_queries::insert_egress_policy(
                &self.db,
                owner,
                &EgressPolicyInput {
                    alias,
                    policy,
                    upstream_proxy_id,
                },
            )
            .await?
            .id
        };
        spec.egress = None;
        spec.egress_policy = Some(id);
        Ok(())
    }
}
