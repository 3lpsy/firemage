use ipnet::IpNet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EgressPolicy {
    #[serde(default = "default_inherit_upstream")]
    pub inherit_upstream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<UpstreamProxy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpProxy>,
    #[serde(default)]
    pub tunnels: Vec<TcpTunnel>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpProxy {
    #[serde(default = "default_proxy_port")]
    pub port: u16,
    #[serde(default)]
    pub rules: Vec<HttpRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_ca_pem: Option<String>,
}

fn default_proxy_port() -> u16 {
    3128
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpScheme {
    Http,
    Https,
}

impl HttpScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpRule {
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, firemage_request_signing::ValueSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing: Option<firemage_request_signing::SigningConfig>,
    pub host: String,
    pub port: u16,
    pub scheme: HttpScheme,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default = "default_path")]
    pub path_prefix: String,
    #[serde(default)]
    pub allowed_ips: Vec<IpNet>,
}

fn default_path() -> String {
    "/".into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TcpTunnel {
    pub name: String,
    pub listen_port: u16,
    pub target_host: String,
    pub target_port: u16,
    #[serde(default)]
    pub allowed_ips: Vec<IpNet>,
}

impl EgressPolicy {
    pub fn ports(&self) -> Vec<u16> {
        self.http
            .iter()
            .map(|p| p.port)
            .chain(self.tunnels.iter().map(|t| t.listen_port))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamProxy {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<firemage_request_signing::ValueSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<firemage_request_signing::ValueSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_pem: Option<String>,
}

fn default_inherit_upstream() -> bool {
    true
}
impl Default for EgressPolicy {
    fn default() -> Self {
        Self {
            inherit_upstream: true,
            upstream: None,
            http: None,
            tunnels: Vec::new(),
        }
    }
}
impl EgressPolicy {
    pub fn secret_names(&self) -> Vec<&str> {
        let mut sources = Vec::new();
        if let Some(http) = &self.http {
            for rule in &http.rules {
                sources.extend(rule.headers.values());
                if let Some(signing) = &rule.signing {
                    sources.extend(signing.sources());
                }
            }
        }
        if let Some(upstream) = &self.upstream {
            sources.extend(upstream.username.iter());
            sources.extend(upstream.password.iter());
        }
        let mut names: Vec<_> = sources
            .into_iter()
            .filter_map(|source| source.secret_name())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}
