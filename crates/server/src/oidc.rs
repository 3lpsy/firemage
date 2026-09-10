use crate::{App, error::Result};
use anyhow::Context;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exchange {
    pub id_token: String,
}
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: u64,
    iat: u64,
    iss: String,
    aud: Value,
    azp: Option<String>,
    nonce: Option<String>,
}
pub(crate) async fn discovery(app: &App) -> anyhow::Result<(reqwest::Client, Value)> {
    let issuer = app
        .runtime
        .config
        .oidc_issuer
        .as_deref()
        .context("OIDC is not configured")?;
    let url = reqwest::Url::parse(issuer)?;
    crate::ensure!(
        is_secure_url(&url) && url.query().is_none() && url.fragment().is_none(),
        "OIDC issuer must be HTTPS without query or fragment"
    );
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15));
    if let Some(path) = &app.runtime.config.oidc_ca_cert {
        anyhow::ensure!(
            tokio::fs::metadata(path).await?.len() <= 1024 * 1024,
            "OIDC CA certificate exceeds 1 MiB"
        );
        let pem = tokio::fs::read(path)
            .await
            .context("cannot read OIDC CA certificate")?;
        let certificates =
            reqwest::Certificate::from_pem_bundle(&pem).context("invalid OIDC CA certificate")?;
        anyhow::ensure!(
            !certificates.is_empty(),
            "invalid OIDC CA certificate: no PEM certificates"
        );
        for certificate in certificates {
            builder = builder.add_root_certificate(certificate);
        }
    }
    let client = builder
        .build()
        .context("cannot configure OIDC HTTPS client")?;
    let response = client
        .get(format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        ))
        .send()
        .await?
        .error_for_status()?;
    let document: Value = bounded_json(response).await?;
    crate::ensure!(
        document["issuer"].as_str() == Some(issuer),
        "OIDC discovery issuer mismatch"
    );
    Ok((client, document))
}
pub(crate) fn endpoint<'a>(document: &'a Value, name: &str) -> anyhow::Result<&'a str> {
    let endpoint = document[name]
        .as_str()
        .context(format!("OIDC provider has no {name}"))?;
    crate::ensure!(
        is_secure_url(&reqwest::Url::parse(endpoint)?),
        "OIDC endpoints must use HTTPS"
    );
    Ok(endpoint)
}
pub async fn configuration(State(app): State<App>) -> Result<Json<Value>> {
    let (_, doc) = discovery(&app).await?;
    Ok(Json(
        json!({"issuer":doc["issuer"],"client_id":app.runtime.config.oidc_client_id.as_deref().context("OIDC client ID is not configured")?,"device_authorization_endpoint":endpoint(&doc,"device_authorization_endpoint")?,"token_endpoint":endpoint(&doc,"token_endpoint")?}),
    ))
}
pub async fn exchange(
    State(app): State<App>,
    Json(input): Json<Exchange>,
) -> Result<Json<firemage_wire::Token>> {
    crate::login::budget(&app).await?;
    crate::ensure!(input.id_token.len() <= 32768, "OIDC token too large");
    let (client, doc) = discovery(&app).await?;
    let user = verify(&app, &client, &doc, &input.id_token, None).await?;
    Ok(Json(crate::login::session(&app, &user.id).await?))
}
pub(crate) async fn verify(
    app: &App,
    client: &reqwest::Client,
    doc: &Value,
    id_token: &str,
    nonce: Option<&str>,
) -> anyhow::Result<firemage_orm::users::Model> {
    anyhow::ensure!(id_token.len() <= 32768, "OIDC token too large");
    let response = client
        .get(endpoint(doc, "jwks_uri")?)
        .send()
        .await?
        .error_for_status()?;
    let keys: jsonwebtoken::jwk::JwkSet = bounded_json(response).await?;
    let header = jsonwebtoken::decode_header(id_token).map_err(anyhow::Error::from)?;
    crate::ensure!(
        matches!(
            header.alg,
            jsonwebtoken::Algorithm::RS256 | jsonwebtoken::Algorithm::ES256
        ),
        "unsupported OIDC signing algorithm"
    );
    let key = keys
        .find(header.kid.as_deref().context("OIDC token missing key id")?)
        .context("OIDC signing key not found")?;
    let client_id = app
        .runtime
        .config
        .oidc_client_id
        .as_deref()
        .context("OIDC client ID is not configured")?;
    let mut validation = jsonwebtoken::Validation::new(header.alg);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&[app
        .runtime
        .config
        .oidc_issuer
        .as_deref()
        .context("OIDC issuer missing")?]);
    validation.set_required_spec_claims(&["exp", "iat", "sub", "iss", "aud"]);
    let claims = jsonwebtoken::decode::<Claims>(
        id_token,
        &jsonwebtoken::DecodingKey::from_jwk(key).map_err(anyhow::Error::from)?,
        &validation,
    )
    .map_err(|_| anyhow::anyhow!("invalid OIDC identity token"))?
    .claims;
    let now = firemage_queries::now() as u64;
    crate::ensure!(
        claims.iat <= now + 60 && now.saturating_sub(claims.iat) <= 300,
        "OIDC identity token must be freshly issued"
    );
    crate::ensure!(
        !claims.sub.is_empty() && claims.sub.len() <= 255,
        "invalid OIDC subject"
    );
    if claims.aud.as_array().is_some_and(|a| a.len() > 1) || claims.azp.is_some() {
        crate::ensure!(
            claims.azp.as_deref() == Some(client_id),
            "OIDC authorized party mismatch"
        );
    }
    if let Some(nonce) = nonce {
        anyhow::ensure!(
            claims.nonce.as_deref() == Some(nonce),
            "OIDC nonce mismatch"
        );
    }
    let user = firemage_queries::user_by_subject(&app.runtime.db, &claims.sub, &claims.iss)
        .await?
        .context("OIDC identity is not linked to a user")?;
    anyhow::ensure!(
        !user.disabled,
        "OIDC identity is not linked to an active user"
    );
    Ok(user)
}

fn is_secure_url(url: &reqwest::Url) -> bool {
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return false;
    }
    if cfg!(test) && url.scheme() == "http" && url.host_str() == Some("127.0.0.1") {
        return true;
    }
    url.scheme() == "https"
}

pub(crate) async fn bounded_json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> anyhow::Result<T> {
    anyhow::ensure!(
        response.content_length().is_none_or(|size| size <= 131072),
        "OIDC response too large"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            bytes.len() + chunk.len() <= 131072,
            "OIDC response too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&bytes)?)
}
