use crate::Runtime;
use firemage_wire::{EgressPolicy, EgressPolicyInput, UpstreamProxy, UpstreamProxyInput};

impl Runtime {
    pub(crate) async fn resolve_catalog_policy(
        &self,
        owner: &str,
        row: &firemage_orm::egress_policies::Model,
    ) -> anyhow::Result<EgressPolicy> {
        let mut policy: EgressPolicy = serde_json::from_str(&row.spec)?;
        if let Some(id) = &row.upstream_proxy_id {
            let proxy = firemage_queries::upstream_proxy(&self.db, owner, id).await?;
            policy.upstream = Some(self.resolve_catalog_proxy(owner, &proxy).await?);
        } else if policy.inherit_upstream {
            policy.upstream = self.config.egress_upstream.clone();
        }
        Ok(policy)
    }
    pub(crate) async fn resolve_catalog_proxy(
        &self,
        owner: &str,
        row: &firemage_orm::upstream_proxies::Model,
    ) -> anyhow::Result<UpstreamProxy> {
        let mut proxy: UpstreamProxy = serde_json::from_str(&row.spec)?;
        if let Some(name) = &row.ca_secret {
            proxy.ca_pem = Some(self.secrets().await?.resolve(owner, name).await?);
        }
        proxy.validate()?;
        if let Some(pem) = &proxy.ca_pem {
            firemage_egress_proxy::validate_ca_pem(pem)?;
        }
        Ok(proxy)
    }
    pub(crate) async fn validate_policy_input(
        &self,
        owner: &str,
        input: &EgressPolicyInput,
    ) -> anyhow::Result<()> {
        input.validate()?;
        let mut policy = input.policy.clone();
        if let Some(id) = &input.upstream_proxy_id {
            let proxy = firemage_queries::upstream_proxy(&self.db, owner, id).await?;
            policy.upstream = Some(self.resolve_catalog_proxy(owner, &proxy).await?);
        } else if policy.inherit_upstream {
            policy.upstream = self.config.egress_upstream.clone();
        }
        for name in policy.secret_names() {
            self.secrets().await?.resolve(owner, name).await?;
        }
        policy.validate()?;
        if let Some(pem) = policy
            .http
            .as_ref()
            .and_then(|http| http.upstream_ca_pem.as_deref())
        {
            firemage_egress_proxy::validate_ca_pem(pem)?;
        }
        Ok(())
    }
    pub(crate) async fn validate_proxy_input(
        &self,
        owner: &str,
        input: &UpstreamProxyInput,
    ) -> anyhow::Result<()> {
        input.validate()?;
        let mut proxy = input.proxy.clone();
        if let Some(name) = &input.ca_secret {
            proxy.ca_pem = Some(self.secrets().await?.resolve(owner, name).await?);
        }
        for source in proxy.username.iter().chain(proxy.password.iter()) {
            if let Some(name) = source.secret_name() {
                self.secrets().await?.resolve(owner, name).await?;
            }
        }
        proxy.validate()?;
        if let Some(pem) = &proxy.ca_pem {
            firemage_egress_proxy::validate_ca_pem(pem)?;
        }
        Ok(())
    }
}
