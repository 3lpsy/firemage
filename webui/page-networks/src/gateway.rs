use ipnet::Ipv4Net;
use std::net::Ipv4Addr;

#[derive(Clone)]
pub struct Gateway {
    pub value: String,
    automatic: bool,
}

impl Gateway {
    pub fn new(existing: Option<&str>, subnet: &str) -> Self {
        Self {
            value: existing
                .map(str::to_owned)
                .or_else(|| suggest(subnet))
                .unwrap_or_default(),
            automatic: existing.is_none(),
        }
    }

    pub fn update_subnet(&mut self, subnet: &str) {
        if self.automatic
            && let Some(suggestion) = suggest(subnet)
        {
            self.value = suggestion;
        }
    }

    pub fn edit(&mut self, value: String) {
        self.value = value;
        self.automatic = false;
    }
}

fn suggest(subnet: &str) -> Option<String> {
    let network: Ipv4Net = subnet.parse().ok()?;
    if network.prefix_len() > 30 {
        return None;
    }
    Some(Ipv4Addr::from(u32::from(network.network()).checked_add(1)?).to_string())
}
