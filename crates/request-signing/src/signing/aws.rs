use crate::{SecretResolver, SigningConfig, resolve};
use aws_sigv4::{
    http_request::{
        PayloadChecksumKind, PercentEncodingMode, SignableBody, SignableRequest, SigningSettings,
        UriPathNormalizationMode, sign,
    },
    sign::v4,
};
use http::HeaderMap;
use std::time::SystemTime;

pub(super) async fn apply(
    resolver: &dyn SecretResolver,
    config: &SigningConfig,
    method: &str,
    url: &str,
    headers: &mut HeaderMap,
    body: &[u8],
    now: SystemTime,
) -> anyhow::Result<()> {
    let SigningConfig::AwsSigv4 {
        region,
        service,
        access_key,
        secret_key,
        session_token,
    } = config
    else {
        unreachable!()
    };
    let access_key = resolve(access_key, resolver).await?;
    let secret_key = resolve(secret_key, resolver).await?;
    let session_token = match session_token {
        Some(value) => Some(resolve(value, resolver).await?),
        None => None,
    };
    let identity = aws_credential_types::Credentials::new(
        access_key,
        secret_key,
        session_token,
        None,
        "firemage-vault",
    )
    .into();
    for name in [
        "authorization",
        "x-amz-date",
        "x-amz-security-token",
        "x-amz-content-sha256",
    ] {
        headers.remove(name);
    }
    let mut settings = SigningSettings::default();
    if service == "s3" {
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        settings.uri_path_normalization_mode = UriPathNormalizationMode::Disabled;
        settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
    }
    let params = v4::SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name(service)
        .time(now)
        .settings(settings)
        .build()
        .map_err(|_| anyhow::anyhow!("invalid AWS signing configuration"))?
        .into();
    let header_values = headers
        .iter()
        .map(|(name, value)| {
            Ok((
                name.as_str(),
                value
                    .to_str()
                    .map_err(|_| anyhow::anyhow!("invalid signing header"))?,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let request = SignableRequest::new(
        method,
        url,
        header_values.into_iter(),
        SignableBody::Bytes(body),
    )
    .map_err(|_| anyhow::anyhow!("invalid AWS signing request"))?;
    let (instructions, _) = sign(request, &params)
        .map_err(|_| anyhow::anyhow!("AWS request signing failed"))?
        .into_parts();
    let mut request = http::Request::new(());
    instructions.apply_to_request_http1x(&mut request);
    for (name, value) in request.headers() {
        super::apply::set_header(
            headers,
            name.as_str(),
            value
                .to_str()
                .map_err(|_| anyhow::anyhow!("invalid signature header"))?,
        )?;
    }
    Ok(())
}
