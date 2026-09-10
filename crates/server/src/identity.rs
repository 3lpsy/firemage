use crate::{
    App,
    error::{Error, Result},
};
use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{StatusCode, request::Parts},
};

pub struct Identity {
    pub user: firemage_orm::users::Model,
    pub credential: firemage_orm::credentials::Model,
}
impl FromRequestParts<App> for Identity {
    type Rejection = Error;
    async fn from_request_parts(parts: &mut Parts, app: &App) -> Result<Self> {
        let bearer = parts.headers.get("authorization");
        let browser = bearer.is_none();
        let token = if let Some(value) = bearer {
            value
                .to_str()
                .ok()
                .and_then(|value| value.strip_prefix("Bearer "))
        } else {
            crate::browser::cookie(&parts.headers, crate::browser::SESSION_COOKIE)
        }
        .ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "login required".into()))?;
        if browser {
            if parts.uri.path().starts_with("/v1/auth/") {
                return Err(Error(
                    StatusCode::UNAUTHORIZED,
                    "bearer token required".into(),
                ));
            }
            if !matches!(
                parts.method,
                axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
            ) {
                crate::browser::ensure_csrf(app, &parts.headers, token)?;
            }
        }
        let (user, credential) = firemage_auth::authenticate(&app.runtime.db, token)
            .await
            .map_err(|_| Error(StatusCode::UNAUTHORIZED, "invalid or expired token".into()))?;
        if browser && credential.kind != "session" {
            return Err(Error(
                StatusCode::UNAUTHORIZED,
                "browser session required".into(),
            ));
        }
        Ok(Self { user, credential })
    }
}
impl Identity {
    pub fn ensure_admin(&self) -> Result<()> {
        if !self.user.admin {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "administrator required for host resource access".into(),
            ));
        }
        Ok(())
    }
    pub fn ensure_session(&self) -> Result<()> {
        if self.credential.kind != "session" {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "session login required".into(),
            ));
        }
        Ok(())
    }
}
pub async fn me(identity: Identity, State(_): State<App>) -> Json<firemage_wire::User> {
    Json(firemage_wire::User {
        id: identity.user.id,
        username: identity.user.username,
        admin: identity.user.admin,
        disabled: identity.user.disabled,
        oidc_subject: identity.user.oidc_subject,
        oidc_issuer: identity.user.oidc_issuer,
    })
}
