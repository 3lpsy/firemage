use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Args, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Server {
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
    #[arg(long, env = "FIREMAGE_UNIX_SOCKET")]
    pub unix_socket: Option<PathBuf>,
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
            database: self.database.or(file.database),
            data_dir: self.data_dir.or(file.data_dir),
            firecracker: self.firecracker.or(file.firecracker),
            session_ttl: self.session_ttl.or(file.session_ttl),
            oidc_issuer: self.oidc_issuer.or(file.oidc_issuer),
            oidc_client_id: self.oidc_client_id.or(file.oidc_client_id),
        }
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
