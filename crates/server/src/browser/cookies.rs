use crate::{
    App,
    error::{Error, Result},
};
use axum::{
    http::{HeaderMap, StatusCode, header},
    response::Response,
};

pub(crate) const SESSION_COOKIE: &str = "firemage_session";
pub(super) const OIDC_COOKIE: &str = "firemage_oidc";

pub(crate) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .flat_map(|header| header.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .filter(|(key, value)| *key == name && !value.is_empty())
        .map(|(_, value)| value);
    let first = values.next()?;
    // Reject ambiguous cookies rather than choosing an attacker-controlled path.
    (values.next().is_none() && first.len() <= 128).then_some(first)
}

pub(super) fn origin(app: &App) -> Result<String> {
    let config = &app.runtime.config;
    let value = config.public_url.clone().unwrap_or_else(|| {
        format!(
            "{}://{}",
            if config.tls_cert.is_some() {
                "https"
            } else {
                "http"
            },
            config.listen.as_deref().unwrap_or("127.0.0.1:8080")
        )
    });
    let url = reqwest::Url::parse(&value).map_err(anyhow::Error::from)?;
    crate::ensure!(
        url.scheme() == "https"
            || (url.scheme() == "http"
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
        "browser login requires an HTTPS public URL or loopback HTTP"
    );
    Ok(url.origin().ascii_serialization())
}

pub(crate) fn ensure_origin(app: &App, headers: &HeaderMap) -> Result<()> {
    let supplied = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    if supplied != Some(origin(app)?.as_str()) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "same-origin browser request required".into(),
        ));
    }
    Ok(())
}

pub(crate) fn csrf(token: &str) -> String {
    firemage_auth::token_hash(&format!("firemage-browser-csrf:{token}"))
}

pub(crate) fn ensure_csrf(app: &App, headers: &HeaderMap, token: &str) -> Result<()> {
    ensure_origin(app, headers)?;
    let expected = csrf(token);
    if headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        != Some(&expected)
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "browser CSRF token required".into(),
        ));
    }
    Ok(())
}

pub(super) fn set_cookie(app: &App, name: &str, token: &str, ttl: i64) -> Result<String> {
    let secure = if origin(app)?.starts_with("https:") {
        "; Secure"
    } else {
        ""
    };
    let same_site = if name == OIDC_COOKIE { "Lax" } else { "Strict" };
    Ok(format!(
        "{name}={token}; Path=/; HttpOnly; SameSite={same_site}; Max-Age={ttl}{secure}"
    ))
}

pub(crate) async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
        ("cache-control", "no-store"),
    ] {
        headers
            .entry(axum::http::HeaderName::from_static(name))
            .or_insert(axum::http::HeaderValue::from_static(value));
    }
    response
}
