use crate::{RegistryCredentials, RegistryOptions, options::https_url};
use anyhow::{Context, ensure};
use oci_spec::distribution::Reference;
use reqwest::{Client, Response, header};
use url::Url;

pub(crate) struct Registry {
    pub(super) client: Client,
    pub(super) reference: Reference,
    pub(super) origin: Url,
    pub(super) options: RegistryOptions,
    pub(super) authorization: Option<header::HeaderValue>,
}

impl Registry {
    pub(crate) fn new(image: &str, options: &RegistryOptions) -> anyhow::Result<Self> {
        options.validate()?;
        ensure!(image.len() <= 2048, "OCI image reference is too long");
        let reference: Reference = image.parse().context("invalid OCI image reference")?;
        if let Some(digest) = reference.digest() {
            super::ensure_digest(digest)?;
        }
        let origin = https_url(&format!("https://{}/", reference.resolve_registry()))?;
        let mut builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(600))
            .user_agent(concat!("firemage/", env!("CARGO_PKG_VERSION")));
        if let Some(pem) = &options.ca_pem {
            let certificates = reqwest::Certificate::from_pem_bundle(pem.as_bytes())
                .map_err(|_| anyhow::anyhow!("invalid registry CA certificate"))?;
            ensure!(!certificates.is_empty(), "registry CA bundle is empty");
            for cert in certificates {
                builder = builder.add_root_certificate(cert);
            }
        }
        let authorization = match &options.credentials {
            RegistryCredentials::Bearer { token } => Some(super::auth::bearer(token)?),
            _ => None,
        };
        Ok(Self {
            client: builder.build()?,
            reference,
            origin,
            options: options.clone(),
            authorization,
        })
    }

    pub(super) fn url(&self, kind: &str, digest: &str) -> anyhow::Result<Url> {
        super::ensure_digest(digest)?;
        Ok(self.origin.join(&format!(
            "v2/{}/{kind}/{digest}",
            self.reference.repository()
        ))?)
    }

    pub(super) fn tag_url(&self, tag: &str) -> anyhow::Result<Url> {
        ensure!(
            !tag.is_empty()
                && tag.len() <= 128
                && (tag.as_bytes()[0].is_ascii_alphanumeric() || tag.starts_with('_'))
                && tag
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
            "invalid OCI tag"
        );
        Ok(self.origin.join(&format!(
            "v2/{}/manifests/{tag}",
            self.reference.repository()
        ))?)
    }

    pub(super) async fn get(&mut self, url: Url, redirects: bool) -> anyhow::Result<Response> {
        let mut response = self.request(&url, true).await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.authenticate(response.headers()).await?;
            response = self.request(&url, true).await?;
        }
        let mut current = url;
        for _ in 0..5 {
            if !response.status().is_redirection() {
                break;
            }
            ensure!(
                redirects,
                "registry manifest and authentication redirects are not allowed"
            );
            let location = response
                .headers()
                .get(header::LOCATION)
                .context("registry redirect has no location")?
                .to_str()
                .context("invalid registry redirect")?;
            let next = current
                .join(location)
                .map_err(|_| anyhow::anyhow!("invalid registry redirect"))?;
            let next = https_url(next.as_str())?;
            // Signed CDN URLs are supported; credentials stay on the registry origin.
            response = self
                .request(&next, next.origin() == self.origin.origin())
                .await?;
            current = next;
        }
        ensure!(
            response.status().is_success(),
            "OCI registry request failed with HTTP {}",
            response.status().as_u16()
        );
        Ok(response)
    }

    async fn request(&self, url: &Url, authenticated: bool) -> anyhow::Result<Response> {
        let mut request = self
            .client
            .get(url.clone())
            .header(header::ACCEPT, super::manifest::ACCEPT);
        if authenticated && let Some(value) = &self.authorization {
            request = request.header(header::AUTHORIZATION, value.clone());
        }
        request
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("OCI registry connection failed"))
    }
}
