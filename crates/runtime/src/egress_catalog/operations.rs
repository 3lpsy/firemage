use crate::Runtime;
use firemage_wire::{
    EgressPolicyInput, EgressPolicyUpdate, UpstreamProxyInput, UpstreamProxyUpdate, VmSpec,
};

impl Runtime {
    pub async fn create_egress_policy(
        &self,
        owner: &str,
        input: EgressPolicyInput,
    ) -> anyhow::Result<firemage_orm::egress_policies::Model> {
        let _guard = self.lock("egress-catalog").await;
        self.validate_policy_input(owner, &input).await?;
        firemage_queries::insert_egress_policy(&self.db, owner, &input).await
    }
    pub async fn create_upstream_proxy(
        &self,
        owner: &str,
        input: UpstreamProxyInput,
    ) -> anyhow::Result<firemage_orm::upstream_proxies::Model> {
        let _guard = self.lock("egress-catalog").await;
        self.validate_proxy_input(owner, &input).await?;
        firemage_queries::insert_upstream_proxy(&self.db, owner, &input).await
    }
    pub async fn delete_egress_policy(&self, owner: &str, id: &str) -> anyhow::Result<()> {
        let _guard = self.lock("egress-catalog").await;
        firemage_queries::delete_egress_policy(&self.db, owner, id).await
    }
    pub async fn delete_upstream_proxy(&self, owner: &str, id: &str) -> anyhow::Result<()> {
        let _guard = self.lock("egress-catalog").await;
        firemage_queries::delete_upstream_proxy(&self.db, owner, id).await
    }
    pub async fn update_egress_policy(
        &self,
        owner: &str,
        id: &str,
        input: EgressPolicyUpdate,
    ) -> anyhow::Result<firemage_orm::egress_policies::Model> {
        let _guard = self.lock("egress-catalog").await;
        let previous = firemage_queries::egress_policy(&self.db, owner, id).await?;
        if u64::try_from(previous.revision)? != input.revision {
            return Err(firemage_queries::CatalogRevisionConflict.into());
        }
        self.validate_policy_input(owner, &input.input).await?;
        let mut proposed = previous.clone();
        proposed.spec = serde_json::to_string(&input.input.policy)?;
        proposed
            .upstream_proxy_id
            .clone_from(&input.input.upstream_proxy_id);
        let rows = firemage_queries::policy_vms(&self.db, owner, id).await?;
        if previous.spec == proposed.spec
            && previous.upstream_proxy_id == proposed.upstream_proxy_id
            && rows.iter().all(|row| !self.is_egress_recovery_pending(row))
        {
            return firemage_queries::update_egress_policy(&self.db, owner, id, &input).await;
        }
        let policy = self.resolve_catalog_policy(owner, &proposed).await?;
        let _vms = self.lock_egress_vms(&rows).await;
        let mut changes = Vec::new();
        for row in rows {
            if let Some(change) = self.live_change(row, Some(policy.clone())).await? {
                changes.push(change);
            }
        }
        self.apply_egress_changes(
            changes,
            firemage_queries::update_egress_policy(&self.db, owner, id, &input),
        )
        .await
    }
    pub async fn update_upstream_proxy(
        &self,
        owner: &str,
        id: &str,
        input: UpstreamProxyUpdate,
    ) -> anyhow::Result<firemage_orm::upstream_proxies::Model> {
        let _guard = self.lock("egress-catalog").await;
        let previous = firemage_queries::upstream_proxy(&self.db, owner, id).await?;
        if u64::try_from(previous.revision)? != input.revision {
            return Err(firemage_queries::CatalogRevisionConflict.into());
        }
        self.validate_proxy_input(owner, &input.input).await?;
        let mut proposed = previous.clone();
        proposed.spec = serde_json::to_string(&input.input.proxy)?;
        proposed.ca_secret.clone_from(&input.input.ca_secret);
        let rows = firemage_queries::proxy_vms(&self.db, owner, id).await?;
        if previous.spec == proposed.spec
            && previous.ca_secret == proposed.ca_secret
            && rows.iter().all(|row| !self.is_egress_recovery_pending(row))
        {
            return firemage_queries::update_upstream_proxy(&self.db, owner, id, &input).await;
        }
        let proxy = self.resolve_catalog_proxy(owner, &proposed).await?;
        let _vms = self.lock_egress_vms(&rows).await;
        let mut changes = Vec::new();
        for row in rows {
            let spec: VmSpec = serde_json::from_str(&row.spec)?;
            let policy_id = spec
                .egress_policy
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("missing policy binding"))?;
            let stored = firemage_queries::egress_policy(&self.db, owner, policy_id).await?;
            let mut policy: firemage_wire::EgressPolicy = serde_json::from_str(&stored.spec)?;
            policy.upstream = Some(proxy.clone());
            if let Some(change) = self.live_change(row, Some(policy)).await? {
                changes.push(change);
            }
        }
        self.apply_egress_changes(
            changes,
            firemage_queries::update_upstream_proxy(&self.db, owner, id, &input),
        )
        .await
    }
    pub async fn assign_egress_policy(
        &self,
        owner: &str,
        id: &str,
        policy_id: Option<String>,
    ) -> anyhow::Result<firemage_wire::Vm> {
        let _catalog = self.lock("egress-catalog").await;
        let _vm = self.lock(id).await;
        let row = firemage_queries::vm(&self.db, owner, id).await?;
        let mut spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.egress_policy == policy_id
            && spec.egress.is_none()
            && !self.is_egress_recovery_pending(&row)
        {
            return crate::view(row);
        }
        spec.egress_policy = policy_id.clone();
        spec.egress = None;
        spec.validate()?;
        self.validate_dependencies(owner, &spec).await?;
        let policy = self.effective_egress(owner, &spec).await?;
        let change = self.live_change(row, policy).await?;
        let saved = self
            .apply_egress_changes(
                change.into_iter().collect(),
                firemage_queries::set_vm_egress_policy(
                    &self.db,
                    owner,
                    id,
                    policy_id.as_deref(),
                    spec.egress_http_port,
                ),
            )
            .await?;
        crate::view(saved)
    }
}
