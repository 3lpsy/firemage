use crate::{SigningConfig, ValueSource};
use hmac::{Hmac, Mac};
use http::HeaderMap;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::SystemTime};

#[async_trait::async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, name: &str) -> anyhow::Result<String>;
}
pub async fn resolve(
    source: &ValueSource,
    resolver: &dyn SecretResolver,
) -> anyhow::Result<String> {
    match source {
        ValueSource::Literal(value) => Ok(value.clone()),
        ValueSource::Secret { secret, prefix } => {
            let value = resolver
                .resolve(secret)
                .await
                .map_err(|_| anyhow::anyhow!("outbound credential unavailable"))?;
            Ok(format!("{prefix}{value}"))
        }
    }
}
pub async fn apply(
    resolver: &dyn SecretResolver,
    injections: &BTreeMap<String, ValueSource>,
    signing: Option<&SigningConfig>,
    method: &str,
    url: &str,
    headers: &mut HeaderMap,
    body: &[u8],
) -> anyhow::Result<()> {
    crate::validate(injections, signing)?;
    http::Method::from_bytes(method.as_bytes())
        .map_err(|_| anyhow::anyhow!("invalid signing method"))?;
    let parsed = url::Url::parse(url).map_err(|_| anyhow::anyhow!("invalid signing URL"))?;
    anyhow::ensure!(
        matches!(parsed.scheme(), "http" | "https")
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.fragment().is_none(),
        "invalid signing URL"
    );

    for (name, source) in injections {
        let value = resolve(source, resolver).await?;
        set_header(headers, name, &value)?;
    }
    if let Some(signing) = signing {
        match signing {
            SigningConfig::AwsSigv4 { .. } => {
                super::aws::apply(
                    resolver,
                    signing,
                    method,
                    url,
                    headers,
                    body,
                    SystemTime::now(),
                )
                .await?
            }
            SigningConfig::HmacSha256 {
                key,
                header,
                prefix,
            } => {
                let timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs()
                    .to_string();
                let url =
                    url::Url::parse(url).map_err(|_| anyhow::anyhow!("invalid signing URL"))?;
                let target = match url.query() {
                    Some(query) => format!("{}?{query}", url.path()),
                    None => url.path().to_owned(),
                };
                let canonical = format!(
                    "{method}\n{target}\n{timestamp}\n{}",
                    hex::encode(Sha256::digest(body))
                );
                let key = resolve(key, resolver).await?;
                let signature = hmac(key.as_bytes(), canonical.as_bytes());
                set_header(headers, "x-firemage-date", &timestamp)?;
                set_header(
                    headers,
                    header,
                    &format!("{prefix}{}", hex::encode(signature)),
                )?;
            }
        }
    }
    Ok(())
}
pub(super) fn hmac(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(value);
    mac.finalize().into_bytes().to_vec()
}
pub(super) fn set_header(headers: &mut HeaderMap, name: &str, value: &str) -> anyhow::Result<()> {
    let name = http::header::HeaderName::from_bytes(name.as_bytes())
        .map_err(|_| anyhow::anyhow!("invalid outbound header"))?;
    let mut value = http::header::HeaderValue::from_str(value)
        .map_err(|_| anyhow::anyhow!("invalid outbound credential value"))?;
    value.set_sensitive(true);
    headers.insert(name, value);
    Ok(())
}
