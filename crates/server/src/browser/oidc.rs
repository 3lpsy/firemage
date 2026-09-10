use super::{
    cookies::{OIDC_COOKIE, cookie, origin, set_cookie},
    handlers,
};
use crate::{
    App,
    error::{Error, Result},
};
use anyhow::Context;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub(super) async fn start(State(app): State<App>) -> Result<Response> {
    crate::login::budget(&app).await?;
    let (_, document) = crate::oidc::discovery(&app).await?;
    let mut url = reqwest::Url::parse(crate::oidc::endpoint(&document, "authorization_endpoint")?)
        .map_err(anyhow::Error::from)?;
    let state = firemage_auth::new_token("oidc");
    let nonce = firemage_auth::new_token("nonce");
    let verifier = firemage_auth::new_token("pkce");
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(verifier.as_bytes()));
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair(
            "client_id",
            app.runtime
                .config
                .oidc_client_id
                .as_deref()
                .context("OIDC client ID missing")?,
        )
        .append_pair("redirect_uri", &callback_url(&app)?)
        .append_pair("scope", "openid profile")
        .append_pair("state", &state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    app.browser.insert(&state, nonce, verifier);
    Ok((
        [(
            header::SET_COOKIE,
            set_cookie(&app, OIDC_COOKIE, &state, 300)?,
        )],
        Redirect::to(url.as_str()),
    )
        .into_response())
}

#[derive(Deserialize)]
pub(super) struct Callback {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
}

pub(super) async fn callback(
    State(app): State<App>,
    headers: HeaderMap,
    Query(input): Query<Callback>,
) -> Result<Response> {
    let result = finish(&app, &headers, input).await;
    let mut response = match result {
        Ok(mut response) => {
            *response.status_mut() = StatusCode::SEE_OTHER;
            response
                .headers_mut()
                .insert(header::LOCATION, "/".parse().expect("static location"));
            response
        }
        Err(error) => {
            tracing::warn!(error = %error.1, "browser OIDC login failed");
            Redirect::to("/?login_error=oidc").into_response()
        }
    };
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(&app, OIDC_COOKIE, "", 0)?
            .parse()
            .map_err(anyhow::Error::from)?,
    );
    Ok(response)
}

async fn finish(app: &App, headers: &HeaderMap, input: Callback) -> Result<Response> {
    let state = input
        .state
        .as_deref()
        .filter(|state| state.len() <= 128 && !state.is_empty())
        .ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "OIDC state missing".into()))?;
    crate::ensure!(
        cookie(headers, OIDC_COOKIE) == Some(state),
        "OIDC browser binding mismatch"
    );
    let pending = app
        .browser
        .take(state)
        .context("OIDC login expired or already used")?;
    crate::ensure!(input.error.is_none(), "OIDC provider refused login");
    let code = input
        .code
        .filter(|code| !code.is_empty() && code.len() <= 8192)
        .context("OIDC code missing")?;
    let (client, document) = crate::oidc::discovery(app).await?;
    let client_id = app
        .runtime
        .config
        .oidc_client_id
        .as_deref()
        .context("OIDC client ID missing")?;
    let mut request = client.post(crate::oidc::endpoint(&document, "token_endpoint")?);
    let redirect = callback_url(app)?;
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", &redirect),
        ("code_verifier", &pending.verifier),
    ];
    if let Some(secret) = app.runtime.config.oidc_client_secret.as_deref() {
        request = request.basic_auth(client_id, Some(secret));
    } else {
        form.push(("client_id", client_id));
    }
    let response = request
        .form(&form)
        .send()
        .await
        .map_err(anyhow::Error::from)?
        .error_for_status()
        .map_err(anyhow::Error::from)?;
    let tokens: serde_json::Value = crate::oidc::bounded_json(response).await?;
    let token = tokens["id_token"]
        .as_str()
        .context("OIDC provider omitted identity token")?;
    let user = crate::oidc::verify(app, &client, &document, token, Some(&pending.nonce)).await?;
    handlers::issued(app, &user).await
}

fn callback_url(app: &App) -> Result<String> {
    crate::ensure!(
        app.runtime.config.public_url.is_some(),
        "OIDC browser login requires public-url"
    );
    Ok(format!("{}/v1/browser/oidc/callback", origin(app)?))
}
