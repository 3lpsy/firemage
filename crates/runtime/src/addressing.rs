use crate::Runtime;
use anyhow::Context;
use firemage_wire::{NetworkAttachment, NetworkPolicy, NetworkSpec, VmSpec};
use std::collections::BTreeSet;
use std::net::Ipv4Addr;

impl Runtime {
    async fn address_reservations(
        &self,
        except: Option<&str>,
    ) -> anyhow::Result<(BTreeSet<Ipv4Addr>, BTreeSet<String>)> {
        let mut addresses = BTreeSet::new();
        let mut macs = BTreeSet::new();
        for row in firemage_queries::vms(&self.db, None).await? {
            if Some(row.id.as_str()) == except {
                continue;
            }
            let spec: VmSpec = serde_json::from_str(&row.spec)?;
            if let Some(net) = spec.network {
                addresses.insert(net.address);
                macs.insert(net.mac.to_ascii_lowercase());
            }
        }
        for row in firemage_queries::all_networks(&self.db).await? {
            let net: NetworkSpec = serde_json::from_str(&row.spec)?;
            addresses.extend([net.gateway, net.subnet.network(), net.subnet.broadcast()]);
            if let NetworkPolicy::HostOnly { address } = net.policy {
                addresses.insert(address);
            }
        }
        Ok((addresses, macs))
    }

    pub async fn suggest_network_address(
        &self,
        owner: &str,
        name: &str,
        except: Option<&str>,
    ) -> anyhow::Result<NetworkAttachment> {
        if let Some(id) = except {
            firemage_queries::vm(&self.db, owner, id).await?;
        }
        let row = firemage_queries::network(&self.db, owner, name).await?;
        let net: NetworkSpec = serde_json::from_str(&row.spec)?;
        let (reserved, macs) = self.address_reservations(except).await?;
        let address =
            suggest_address(&net, &reserved).context("network has no unused guest addresses")?;
        let mac = loop {
            let bytes = *uuid::Uuid::new_v4().as_bytes();
            let mac = format!(
                "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]
            );
            if !macs.contains(&mac) {
                break mac;
            }
        };
        Ok(NetworkAttachment {
            network: name.into(),
            address,
            mac,
        })
    }

    pub(crate) async fn ensure_network_address_available(
        &self,
        owner: &str,
        attachment: &NetworkAttachment,
        except: Option<&str>,
    ) -> anyhow::Result<()> {
        let row = firemage_queries::network(&self.db, owner, &attachment.network).await?;
        let net: NetworkSpec = serde_json::from_str(&row.spec)?;
        let (reserved, macs) = self.address_reservations(except).await?;
        anyhow::ensure!(
            net.subnet.contains(&attachment.address) && !reserved.contains(&attachment.address),
            "guest IP is outside the subnet, reserved, or already assigned to a VM"
        );
        anyhow::ensure!(
            !macs.contains(&attachment.mac.to_ascii_lowercase()),
            "MAC address is already assigned to a VM"
        );
        Ok(())
    }
}

fn suggest_address(net: &NetworkSpec, reserved: &BTreeSet<Ipv4Addr>) -> Option<Ipv4Addr> {
    let first = u32::from(net.subnet.network()) + 1;
    let last = u32::from(net.subnet.broadcast()) - 1;
    let preferred = u32::from(net.gateway).saturating_add(1).clamp(first, last);
    (preferred..=last)
        .chain(first..preferred)
        .map(Ipv4Addr::from)
        .find(|ip| *ip != net.gateway && !reserved.contains(ip))
}

#[cfg(test)]
#[path = "addressing_tests.rs"]
mod tests;
