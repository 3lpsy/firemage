use crate::Server;
impl Server {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.session_ttl()?;
        if let Some(path) = &self.firemage_guest_bin_path {
            anyhow::ensure!(
                path.is_absolute()
                    && path.components().all(|part| matches!(
                        part,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )),
                "firemage_guest_bin_path must be an absolute path without traversal"
            );
        }
        anyhow::ensure!(
            self.snapshot_max_bytes() > 0 && self.snapshot_max_bytes() <= 1024 * 1024 * 1024 * 1024,
            "snapshot_max_bytes must be positive and at most 1 TiB"
        );
        if let Some(path) = &self.snapshot_dir {
            anyhow::ensure!(
                path.is_absolute()
                    && path.components().all(|part| matches!(
                        part,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )),
                "snapshot_dir must be an absolute directory without traversal"
            );
        }
        anyhow::ensure!(
            self.asset_max_bytes() > 0 && self.seed_max_bytes() <= isize::MAX as u64 / 2,
            "asset_max_bytes must be positive and leave room for encoded boot inputs"
        );
        // Seed images reserve filesystem overhead and share the ext4 builder's 32 GiB ceiling.
        anyhow::ensure!(
            self.seed_max_bytes().div_ceil(1024 * 1024) * 5 / 4 + 16 <= 32768,
            "asset_max_bytes exceeds the 32 GiB seed disk capacity"
        );
        anyhow::ensure!(
            self.kernel_dir().is_absolute()
                && self.kernel_dir().components().all(|part| matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )),
            "kernel_dir must be an absolute directory without traversal"
        );
        for roots in [&self.local_asset_roots, &self.external_socket_roots]
            .into_iter()
            .flatten()
        {
            anyhow::ensure!(
                roots.len() <= 64 && roots.iter().all(|path| path.is_absolute()),
                "asset/socket roots must be at most 64 absolute directory paths"
            );
        }
        anyhow::ensure!(
            self.asset_dir().is_absolute()
                && self.asset_dir().components().all(|part| matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )),
            "asset_dir must be an absolute directory without traversal"
        );
        self.validate_unix_socket()?;
        anyhow::ensure!(
            self.jailer_uid_base.is_some() == self.jailer_uid_count.is_some(),
            "jailer_uid_base and jailer_uid_count must be set together"
        );
        if let (Some(base), Some(count)) = (self.jailer_uid_base, self.jailer_uid_count) {
            anyhow::ensure!(
                base >= 65536
                    && count > 0
                    && count <= 1048576
                    && base.checked_add(count).is_some_and(|end| end < u32::MAX),
                "invalid reserved jailer UID/GID range"
            );
        }
        if let Some(parent) = &self.jailer_cgroup_parent {
            anyhow::ensure!(
                !parent.is_empty()
                    && parent.split('/').all(|part| !part.is_empty()
                        && part != "."
                        && part != ".."
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))),
                "invalid relative jailer cgroup parent"
            );
        }
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
            &self.jailer,
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
