use crate::{EgressPolicy, HttpRule};
use anyhow::{Result, ensure};
use std::{collections::HashSet, net::IpAddr};

impl EgressPolicy {
    pub fn validate(&self) -> Result<()> {
        if let Some(upstream) = &self.upstream {
            upstream.validate()?;
        }
        ensure!(
            self.tunnels.len() <= 64,
            "at most 64 TCP tunnels are supported"
        );
        let mut ports = HashSet::new();
        for port in self.ports() {
            ensure!(port >= 1024, "egress listener ports must be at least 1024");
            ensure!(ports.insert(port), "egress listener ports must be unique");
        }
        if let Some(http) = &self.http {
            ensure!(
                http.rules.len() <= 256,
                "at most 256 HTTP rules are supported"
            );
            if let Some(pem) = &http.upstream_ca_pem {
                ensure!(
                    pem.len() <= 65536
                        && pem.contains("-----BEGIN CERTIFICATE-----")
                        && !pem.contains("PRIVATE KEY"),
                    "upstream CA must contain public PEM certificates"
                );
            }
            for rule in &http.rules {
                rule.validate()?;
            }
        }
        let mut names = HashSet::new();
        for tunnel in &self.tunnels {
            ensure!(
                !tunnel.name.is_empty()
                    && tunnel.name.len() <= 64
                    && tunnel
                        .name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "tunnel names use 1-64 letters, numbers, hyphens or underscores"
            );
            ensure!(names.insert(&tunnel.name), "tunnel names must be unique");
            ensure!(is_host(&tunnel.target_host), "invalid tunnel target host");
            ensure!(tunnel.target_port > 0, "tunnel target port must be nonzero");
        }
        Ok(())
    }
}

impl HttpRule {
    fn validate(&self) -> Result<()> {
        firemage_request_signing::validate(&self.headers, self.signing.as_ref())?;
        ensure!(
            is_host(&self.host),
            "HTTP rules require an exact hostname or IP address"
        );
        ensure!(self.port > 0, "HTTP rule port must be nonzero");
        ensure!(
            self.path_prefix.starts_with('/')
                && self.path_prefix.len() <= 2048
                && !self.path_prefix.bytes().any(|b| b <= 32
                    || b == b'\\'
                    || b == b'?'
                    || b == b'#'
                    || b == b'%')
                && !self.path_prefix.split('/').any(|p| p == "." || p == ".."),
            "invalid HTTP path prefix"
        );
        for method in &self.methods {
            ensure!(
                matches!(
                    method.as_str(),
                    "GET" | "HEAD" | "POST" | "PUT" | "DELETE" | "OPTIONS" | "PATCH"
                ),
                "unsupported HTTP method"
            );
        }
        Ok(())
    }

    pub fn is_match(
        &self,
        scheme: crate::HttpScheme,
        host: &str,
        port: u16,
        method: &str,
        path: &str,
    ) -> bool {
        self.scheme == scheme
            && self.host.eq_ignore_ascii_case(host)
            && self.port == port
            && (self.methods.is_empty() || self.methods.iter().any(|m| m == method))
            && path.starts_with(&self.path_prefix)
    }
}

fn is_host(host: &str) -> bool {
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

impl crate::UpstreamProxy {
    pub fn validate(&self) -> Result<()> {
        let url = url::Url::parse(&self.url)?;
        ensure!(
            matches!(url.scheme(), "http" | "https" | "socks5")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.path().trim_matches('/').is_empty()
                && url.query().is_none()
                && url.fragment().is_none(),
            "upstream proxy must be an HTTP, HTTPS or SOCKS5 URL without embedded credentials or paths"
        );
        ensure!(
            url.port_or_known_default().is_some() || url.scheme() == "socks5",
            "invalid upstream proxy port"
        );
        ensure!(
            self.username.is_some() == self.password.is_some(),
            "upstream proxy username and password must be set together"
        );
        for source in self.username.iter().chain(self.password.iter()) {
            firemage_request_signing::ensure_source(source)?;
        }
        if let Some(password) = &self.password {
            ensure!(
                matches!(password, crate::ValueSource::Secret {prefix, ..} if prefix.is_empty()),
                "upstream passwords must reference a named secret without a prefix"
            );
        }
        ensure!(url.port() != Some(0), "upstream proxy port must be nonzero");
        if let Some(pem) = &self.ca_pem {
            ensure!(
                pem.len() <= 65536
                    && pem.contains("-----BEGIN CERTIFICATE-----")
                    && !pem.contains("PRIVATE KEY"),
                "proxy CA must contain public certificates"
            );
        }
        Ok(())
    }
}
