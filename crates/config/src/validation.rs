use crate::Server;
impl Server {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.session_ttl()?;
        if let Some(upstream) = &self.egress_upstream {
            upstream.validate()?;
        }
        if let Some(args) = &self.firecracker_args {
            anyhow::ensure!(
                args.len() <= 128
                    && args.iter().all(|arg| arg.len() <= 4096
                        && !arg.contains('\0')
                        && !matches!(arg.as_str(), "--api-sock" | "--no-api")
                        && !arg.starts_with("--api-sock=")),
                "invalid Firecracker arguments or API socket override"
            );
        }
        if let Some(client_id) = &self.oidc_client_id {
            anyhow::ensure!(
                !client_id.trim().is_empty() && client_id.len() <= 1024,
                "invalid OIDC client ID"
            );
        }
        anyhow::ensure!(
            self.tls_cert.is_some() == self.tls_key.is_some(),
            "tls-cert and tls-key must be set together"
        );
        anyhow::ensure!(
            self.unix_socket.is_none() || (self.listen.is_none() && self.tls_cert.is_none()),
            "choose a Unix socket or TCP listener"
        );
        if let Some(listen) = &self.listen {
            let address: std::net::SocketAddr = listen
                .parse()
                .map_err(|_| anyhow::anyhow!("listen must be an IP address and port"))?;
            anyhow::ensure!(
                address.ip().is_loopback() || self.tls_cert.is_some(),
                "public listeners require TLS"
            );
        }
        anyhow::ensure!(
            self.database().starts_with("sqlite:"),
            "database must be a SQLite URL"
        );
        anyhow::ensure!(
            self.oidc_issuer.is_some() == self.oidc_client_id.is_some(),
            "OIDC issuer and client ID must be set together"
        );
        anyhow::ensure!(
            (self.oidc_client_secret.is_none() && self.oidc_ca_cert.is_none())
                || self.oidc_issuer.is_some(),
            "OIDC secret or CA certificate requires an issuer"
        );
        for (name, value) in [
            ("public_url", &self.public_url),
            ("oidc_issuer", &self.oidc_issuer),
        ] {
            if let Some(value) = value {
                let url = url::Url::parse(value)
                    .map_err(|_| anyhow::anyhow!("{name} must be an absolute URL"))?;
                let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
                anyhow::ensure!(
                    url.scheme() == "https"
                        || (name == "public_url" && url.scheme() == "http" && local),
                    "{name} requires HTTPS except on loopback"
                );
                anyhow::ensure!(
                    url.username().is_empty()
                        && url.password().is_none()
                        && url.query().is_none()
                        && url.fragment().is_none(),
                    "{name} cannot contain credentials, query or fragment"
                );
                if name == "public_url" {
                    anyhow::ensure!(
                        url.path() == "/",
                        "public_url must be an origin without a path"
                    );
                }
            }
        }
        for value in [
            &self.data_dir,
            &self.webui_dir,
            &self.oidc_ca_cert,
            &self.firecracker,
            &self.unix_socket,
            &self.tls_cert,
            &self.tls_key,
        ]
        .into_iter()
        .flatten()
        {
            anyhow::ensure!(
                !value.as_os_str().is_empty(),
                "configuration paths cannot be empty"
            );
        }
        Ok(())
    }
}
