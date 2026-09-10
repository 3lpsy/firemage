use anyhow::ensure;
use url::Url;

#[derive(Clone, Default)]
pub enum RegistryCredentials {
    #[default]
    Anonymous,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
}

#[derive(Clone, Default)]
pub struct RegistryOptions {
    pub credentials: RegistryCredentials,
    /// Exact additional HTTPS token endpoint trusted to receive registry credentials.
    pub token_realm: Option<String>,
    pub ca_pem: Option<String>,
}

impl RegistryOptions {
    pub fn validate(&self) -> anyhow::Result<()> {
        match &self.credentials {
            RegistryCredentials::Anonymous => {}
            RegistryCredentials::Basic { username, password } => {
                ensure!(
                    !username.contains(':') && is_valid_secret(username, 256),
                    "invalid registry username"
                );
                ensure!(
                    is_valid_secret(password, 65536),
                    "invalid registry password"
                );
            }
            RegistryCredentials::Bearer { token } => {
                ensure!(
                    !token.is_empty()
                        && token.len() <= 16384
                        && token.bytes().all(|b| b.is_ascii_graphic()),
                    "invalid registry bearer token"
                );
            }
        }
        if let Some(realm) = &self.token_realm {
            let url = https_url(realm)?;
            ensure!(
                url.query().is_none(),
                "registry token realm must not contain a query"
            );
        }
        if let Some(pem) = &self.ca_pem {
            ensure!(pem.len() <= 65536, "registry CA exceeds 64 KiB");
        }
        Ok(())
    }
}

fn is_valid_secret(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

pub(crate) fn https_url(value: &str) -> anyhow::Result<Url> {
    ensure!(value.len() <= 8192, "registry URL is too long");
    let url = Url::parse(value).map_err(|_| anyhow::anyhow!("invalid registry URL"))?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "registry URLs must use HTTPS without userinfo or fragments"
    );
    Ok(url)
}
