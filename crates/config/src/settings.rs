use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Args, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Server {
    #[arg(long, env = "FIREMAGE_ASSET_DIR")]
    pub asset_dir: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_KERNEL_DIR")]
    pub kernel_dir: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_LOCAL_ASSET_ROOTS", value_delimiter = ',')]
    pub local_asset_roots: Option<Vec<PathBuf>>,
    #[arg(long, env = "FIREMAGE_EXTERNAL_SOCKET_ROOTS", value_delimiter = ',')]
    pub external_socket_roots: Option<Vec<PathBuf>>,
    #[arg(long, env = "FIREMAGE_JAILER")]
    pub jailer: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_JAILER_UID_BASE")]
    pub jailer_uid_base: Option<u32>,
    #[arg(long, env = "FIREMAGE_JAILER_UID_COUNT")]
    pub jailer_uid_count: Option<u32>,
    #[arg(long, env = "FIREMAGE_JAILER_CGROUP_PARENT")]
    pub jailer_cgroup_parent: Option<String>,
    #[arg(long, env = "FIREMAGE_ALLOW_TRUSTED_VMS", num_args = 0..=1, default_missing_value = "true")]
    pub allow_trusted_vms: Option<bool>,
    #[arg(long, env = "FIREMAGE_ALLOW_EXTERNAL_VMS", num_args = 0..=1, default_missing_value = "true")]
    pub allow_external_vms: Option<bool>,
    #[arg(long, env = "FIREMAGE_ALLOW_UNRESTRICTED_RAW_API", num_args = 0..=1, default_missing_value = "true")]
    pub allow_unrestricted_raw_api: Option<bool>,
    #[arg(long, env = "FIREMAGE_EGRESS_UPSTREAM", value_parser = parse_upstream)]
    pub egress_upstream: Option<firemage_egress_policy::UpstreamProxy>,
    #[arg(long, env = "FIREMAGE_PUBLIC_URL")]
    pub public_url: Option<String>,
    #[arg(long, env = "FIREMAGE_OIDC_CLIENT_SECRET", hide_env_values = true)]
    pub oidc_client_secret: Option<String>,
    #[arg(long, env = "FIREMAGE_OIDC_CA_CERT")]
    pub oidc_ca_cert: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_WEBUI_DIR")]
    pub webui_dir: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_FIRECRACKER_ARGS", value_delimiter = ',')]
    pub firecracker_args: Option<Vec<String>>,
    #[arg(long, env = "FIREMAGE_TLS_CERT")]
    pub tls_cert: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_TLS_KEY")]
    pub tls_key: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_LISTEN")]
    pub listen: Option<String>,
    /// Serve HTTP on this Unix socket instead of TCP. Its parent must already exist.
    #[arg(long, env = "FIREMAGE_UNIX_SOCKET")]
    pub unix_socket: Option<PathBuf>,
    /// Octal socket permissions: 0600 (default) or 0660 with an explicit group.
    #[arg(long, env = "FIREMAGE_UNIX_SOCKET_MODE")]
    pub unix_socket_mode: Option<String>,
    /// Numeric group that may connect when socket mode is 0660.
    #[arg(long, env = "FIREMAGE_UNIX_SOCKET_GID")]
    pub unix_socket_gid: Option<u32>,
    #[arg(long, env = "FIREMAGE_DATABASE")]
    pub database: Option<String>,
    #[arg(long, env = "FIREMAGE_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_FIRECRACKER")]
    pub firecracker: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_SESSION_TTL")]
    pub session_ttl: Option<u64>,
    #[arg(long, env = "FIREMAGE_OIDC_ISSUER")]
    pub oidc_issuer: Option<String>,
    #[arg(long, env = "FIREMAGE_OIDC_CLIENT_ID")]
    pub oidc_client_id: Option<String>,
}
impl Server {
    pub fn merge(self, file: Self) -> Self {
        Self {
            kernel_dir: self.kernel_dir.or(file.kernel_dir),
            asset_dir: self.asset_dir.or(file.asset_dir),
            local_asset_roots: self.local_asset_roots.or(file.local_asset_roots),
            external_socket_roots: self.external_socket_roots.or(file.external_socket_roots),
            jailer: self.jailer.or(file.jailer),
            jailer_uid_base: self.jailer_uid_base.or(file.jailer_uid_base),
            jailer_uid_count: self.jailer_uid_count.or(file.jailer_uid_count),
            jailer_cgroup_parent: self.jailer_cgroup_parent.or(file.jailer_cgroup_parent),
            allow_trusted_vms: self.allow_trusted_vms.or(file.allow_trusted_vms),
            allow_external_vms: self.allow_external_vms.or(file.allow_external_vms),
            allow_unrestricted_raw_api: self
                .allow_unrestricted_raw_api
                .or(file.allow_unrestricted_raw_api),
            egress_upstream: self.egress_upstream.or(file.egress_upstream),
            public_url: self.public_url.or(file.public_url),
            oidc_client_secret: self.oidc_client_secret.or(file.oidc_client_secret),
            oidc_ca_cert: self.oidc_ca_cert.or(file.oidc_ca_cert),
            webui_dir: self.webui_dir.or(file.webui_dir),
            firecracker_args: self.firecracker_args.or(file.firecracker_args),
            tls_cert: self.tls_cert.or(file.tls_cert),
            tls_key: self.tls_key.or(file.tls_key),
            listen: self.listen.or(file.listen),
            unix_socket: self.unix_socket.or(file.unix_socket),
            unix_socket_mode: self.unix_socket_mode.or(file.unix_socket_mode),
            unix_socket_gid: self.unix_socket_gid.or(file.unix_socket_gid),
            database: self.database.or(file.database),
            data_dir: self.data_dir.or(file.data_dir),
            firecracker: self.firecracker.or(file.firecracker),
            session_ttl: self.session_ttl.or(file.session_ttl),
            oidc_issuer: self.oidc_issuer.or(file.oidc_issuer),
            oidc_client_id: self.oidc_client_id.or(file.oidc_client_id),
        }
    }
    pub fn asset_dir(&self) -> PathBuf {
        self.asset_dir
            .clone()
            .unwrap_or_else(|| "/var/lib/firemage/assets/files".into())
    }
    pub fn kernel_dir(&self) -> PathBuf {
        self.kernel_dir
            .clone()
            .unwrap_or_else(|| "/var/lib/firemage/kernels".into())
    }
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(|| "data".into())
    }
    pub fn database(&self) -> String {
        self.database.clone().unwrap_or_else(|| {
            format!(
                "sqlite://{}/firemage.db?mode=rwc",
                self.data_dir().display()
            )
        })
    }
    pub fn session_ttl(&self) -> anyhow::Result<i64> {
        let ttl = self.session_ttl.unwrap_or(86400);
        anyhow::ensure!(
            (60..=2_592_000).contains(&ttl),
            "session TTL must be 60-2592000 seconds"
        );
        Ok(ttl as i64)
    }
}
#[derive(Debug, Default, Clone, Args, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Client {
    #[arg(long, env = "FIREMAGE_PASSWORD_STDIN", global = true, num_args = 0..=1, default_missing_value = "true")]
    pub password_stdin: Option<bool>,
    #[arg(long, env = "FIREMAGE_USERNAME", global = true)]
    pub username: Option<String>,
    #[arg(long, env = "FIREMAGE_OIDC", global = true, num_args = 0..=1, default_missing_value = "true")]
    pub oidc: Option<bool>,
    #[arg(long, env = "FIREMAGE_CA_CERT", global = true)]
    pub ca_cert: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_URL", global = true)]
    pub url: Option<String>,
    #[arg(long, env = "FIREMAGE_CLIENT_SOCKET", global = true)]
    pub client_socket: Option<PathBuf>,
    #[arg(long, env = "FIREMAGE_AUTH_TOKEN_PATH", global = true)]
    pub auth_token_path: Option<PathBuf>,
    #[arg(
        long,
        env = "FIREMAGE_AUTHTOKEN",
        global = true,
        hide_env_values = true
    )]
    #[serde(skip_serializing)]
    pub authtoken: Option<String>,
    #[arg(long, env = "FIREMAGE_APITOKEN", global = true, hide_env_values = true)]
    #[serde(skip_serializing)]
    pub apitoken: Option<String>,
}
impl Client {
    pub fn merge(self, file: Self) -> Self {
        Self {
            password_stdin: self.password_stdin.or(file.password_stdin),
            username: self.username.or(file.username),
            oidc: self.oidc.or(file.oidc),
            ca_cert: self.ca_cert.or(file.ca_cert),
            url: self.url.or(file.url),
            client_socket: self.client_socket.or(file.client_socket),
            auth_token_path: self.auth_token_path.or(file.auth_token_path),
            authtoken: self.authtoken.or(file.authtoken),
            apitoken: self.apitoken.or(file.apitoken),
        }
    }
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: Server,
    pub client: Client,
}

fn parse_upstream(value: &str) -> Result<firemage_egress_policy::UpstreamProxy, String> {
    let proxy: firemage_egress_policy::UpstreamProxy = toml::from_str(value)
        .map_err(|_| "egress-upstream requires TOML proxy fields".to_owned())?;
    proxy.validate().map_err(|e| e.to_string())?;
    Ok(proxy)
}
