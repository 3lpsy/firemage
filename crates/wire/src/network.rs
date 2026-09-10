use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkSpec {
    pub name: String,
    pub subnet: Ipv4Net,
    pub gateway: Ipv4Addr,
    #[serde(default)]
    pub policy: NetworkPolicy,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NetworkPolicy {
    #[default]
    Isolated,
    HostOnly {
        address: Ipv4Addr,
    },
    FiremageOnly,
    Unrestricted,
}
impl NetworkSpec {
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::ensure_name(&self.name)?;
        anyhow::ensure!(
            self.subnet.prefix_len() <= 30
                && self.subnet.contains(&self.gateway)
                && self.gateway != self.subnet.network()
                && self.gateway != self.subnet.broadcast(),
            "gateway must be a usable address in subnet"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkAttachment {
    pub network: String,
    pub address: Ipv4Addr,
    pub mac: String,
}
