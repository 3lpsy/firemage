use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EgressPolicyInput {
    pub alias: String,
    pub policy: crate::EgressPolicy,
    #[serde(default)]
    pub upstream_proxy_id: Option<String>,
}
impl EgressPolicyInput {
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::ensure_asset_alias(&self.alias)?;
        self.policy.validate()?;
        anyhow::ensure!(
            self.policy.upstream.is_none(),
            "catalog policies must select a reusable upstream proxy"
        );
        if let Some(id) = &self.upstream_proxy_id {
            crate::ensure_asset_id(id)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EgressPolicyUpdate {
    pub revision: u64,
    #[serde(flatten)]
    pub input: EgressPolicyInput,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamProxyInput {
    pub alias: String,
    pub proxy: crate::UpstreamProxy,
    #[serde(default)]
    pub ca_secret: Option<String>,
}
impl UpstreamProxyInput {
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::ensure_asset_alias(&self.alias)?;
        self.proxy.validate()?;
        anyhow::ensure!(
            self.ca_secret.is_none() || self.proxy.ca_pem.is_none(),
            "choose a CA secret or inline public CA, not both"
        );
        if let Some(name) = &self.ca_secret {
            crate::ensure_name(name)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamProxyUpdate {
    pub revision: u64,
    #[serde(flatten)]
    pub input: UpstreamProxyInput,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EgressVmReference {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub state: String,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EgressPolicyReference {
    pub id: String,
    pub alias: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEgressPolicy {
    pub id: String,
    pub owner_id: String,
    pub revision: u64,
    #[serde(flatten)]
    pub input: EgressPolicyInput,
    pub vm_count: usize,
    pub vms: Vec<EgressVmReference>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogUpstreamProxy {
    pub id: String,
    pub owner_id: String,
    pub revision: u64,
    #[serde(flatten)]
    pub input: UpstreamProxyInput,
    pub vm_count: usize,
    pub vms: Vec<EgressVmReference>,
    pub policy_count: usize,
    pub policies: Vec<EgressPolicyReference>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignEgressPolicy {
    pub policy_id: Option<String>,
}
