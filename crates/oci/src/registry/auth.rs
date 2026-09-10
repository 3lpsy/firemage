use super::{Registry, download::read_bounded};
use crate::{RegistryCredentials, options::https_url};
use anyhow::{Context, ensure};
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::Deserialize;

#[derive(Deserialize)]
struct Token {
    token: Option<String>,
    access_token: Option<String>,
}

pub(super) fn bearer(token: &str) -> anyhow::Result<HeaderValue> {
    ensure!(
        !token.is_empty() && token.len() <= 16384 && token.bytes().all(|b| b.is_ascii_graphic()),
        "invalid registry token"
    );
    let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
        .map_err(|_| anyhow::anyhow!("invalid registry token"))?;
    value.set_sensitive(true);
    Ok(value)
}

impl Registry {
    pub(super) async fn authenticate(&mut self, headers: &HeaderMap) -> anyhow::Result<()> {
        ensure!(
            !matches!(self.options.credentials, RegistryCredentials::Bearer { .. }),
            "registry rejected the configured bearer token"
        );
        let mut challenges = Vec::new();
        for value in headers.get_all(header::WWW_AUTHENTICATE) {
            let value = value
                .to_str()
                .context("invalid registry authentication challenge")?;
            ensure!(
                value.len() <= 16384,
                "registry authentication challenge is too large"
            );
            challenges.extend(
                http_auth::parse_challenges(value)
                    .map_err(|_| anyhow::anyhow!("invalid registry authentication challenge"))?,
            );
        }
        if let Some(challenge) = challenges
            .iter()
            .find(|c| c.scheme.eq_ignore_ascii_case("bearer"))
        {
            let parameter = |name: &str| -> anyhow::Result<Option<String>> {
                let values: Vec<_> = challenge
                    .params
                    .iter()
                    .filter(|(key, _)| key.eq_ignore_ascii_case(name))
                    .collect();
                ensure!(
                    values.len() <= 1,
                    "duplicate registry authentication parameter"
                );
                Ok(values.first().map(|(_, value)| value.to_unescaped()))
            };
            let realm =
                https_url(&parameter("realm")?.context("registry bearer challenge has no realm")?)?;
            ensure!(
                realm.query().is_none(),
                "registry token realm must not contain a query"
            );
            let explicitly_trusted = self
                .options
                .token_realm
                .as_deref()
                .map(https_url)
                .transpose()?
                .is_some_and(|trusted| trusted == realm);
            let docker_hub = self.reference.resolve_registry() == "registry-1.docker.io"
                && realm.as_str() == "https://auth.docker.io/token";
            let same_origin = realm.origin() == self.origin.origin();
            ensure!(
                same_origin || explicitly_trusted || docker_hub,
                "registry token realm is on another origin; configure registry.token_realm to trust it"
            );
            let mut request = self.client.get(realm).query(&[(
                "scope",
                format!("repository:{}:pull", self.reference.repository()),
            )]);
            if let Some(service) = parameter("service")? {
                ensure!(service.len() <= 2048, "registry token service is too long");
                request = request.query(&[("service", service)]);
            }
            if let RegistryCredentials::Basic { username, password } = &self.options.credentials {
                request = request.basic_auth(username, Some(password));
            }
            let response = request
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("registry authentication connection failed"))?;
            ensure!(
                response.status().is_success(),
                "registry authentication failed with HTTP {}",
                response.status().as_u16()
            );
            let bytes = read_bounded(response, 65536).await?;
            let token: Token = serde_json::from_slice(&bytes)
                .map_err(|_| anyhow::anyhow!("invalid registry authentication response"))?;
            let token = token
                .token
                .filter(|t| !t.is_empty())
                .or(token.access_token)
                .context("registry authentication response has no token")?;
            self.authorization = Some(bearer(&token)?);
        } else if challenges
            .iter()
            .any(|c| c.scheme.eq_ignore_ascii_case("basic"))
        {
            let RegistryCredentials::Basic { username, password } = &self.options.credentials
            else {
                anyhow::bail!("registry requires username/password credentials");
            };
            let request = self
                .client
                .get(self.origin.clone())
                .basic_auth(username, Some(password))
                .build()?;
            self.authorization = request.headers().get(header::AUTHORIZATION).cloned();
        } else {
            anyhow::bail!("registry authentication challenge is missing or unsupported");
        }
        Ok(())
    }
}
