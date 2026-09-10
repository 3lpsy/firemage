use crate::{SigningConfig, ValueSource};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate(
    headers: &BTreeMap<String, ValueSource>,
    signing: Option<&SigningConfig>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        headers.len() <= 32,
        "at most 32 injected headers are allowed"
    );
    let mut names = BTreeSet::new();
    for (name, source) in headers {
        ensure_header(name)?;
        anyhow::ensure!(
            names.insert(name.to_ascii_lowercase()),
            "duplicate injected header"
        );
        ensure_source(source)?;
    }
    if let Some(signing) = signing {
        for source in signing.sources() {
            ensure_source(source)?;
        }
        match signing {
            SigningConfig::AwsSigv4 {
                region,
                service,
                secret_key,
                session_token,
                ..
            } => {
                for value in [region, service] {
                    anyhow::ensure!(
                        !value.is_empty()
                            && value.len() <= 64
                            && value
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
                        "invalid AWS region or service"
                    );
                }
                anyhow::ensure!(
                    secret_key.secret_name().is_some()
                        && session_token
                            .as_ref()
                            .is_none_or(|v| v.secret_name().is_some()),
                    "AWS credentials must use named secrets"
                );
            }
            SigningConfig::HmacSha256 {
                key,
                header,
                prefix,
            } => {
                ensure_header(header)?;
                anyhow::ensure!(
                    !header.eq_ignore_ascii_case("x-firemage-date"),
                    "signature header cannot replace signing timestamp"
                );
                ensure_text(prefix)?;
                anyhow::ensure!(
                    key.secret_name().is_some(),
                    "HMAC keys must use named secrets"
                );
            }
        }
    }
    Ok(())
}
fn ensure_header(name: &str) -> anyhow::Result<()> {
    http::header::HeaderName::from_bytes(name.as_bytes())
        .map_err(|_| anyhow::anyhow!("invalid injected header name"))?;
    anyhow::ensure!(
        !matches!(
            name.to_ascii_lowercase().as_str(),
            "host"
                | "connection"
                | "content-length"
                | "transfer-encoding"
                | "upgrade"
                | "te"
                | "trailer"
                | "proxy-authorization"
                | "proxy-connection"
                | "keep-alive"
                | "proxy-authenticate"
        ),
        "transport headers cannot be injected"
    );
    Ok(())
}
fn ensure_text(value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        value.len() <= 8192 && !value.contains(['\r', '\n', '\0']),
        "invalid outbound header value"
    );
    Ok(())
}
pub fn ensure_source(value: &ValueSource) -> anyhow::Result<()> {
    match value {
        ValueSource::Literal(value) => ensure_text(value),
        ValueSource::Secret { secret, prefix } => {
            anyhow::ensure!(
                !secret.is_empty()
                    && secret.len() <= 64
                    && secret
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "invalid secret name"
            );
            ensure_text(prefix)
        }
    }
}
