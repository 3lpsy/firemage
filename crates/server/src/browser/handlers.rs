use super::cookies::{SESSION_COOKIE, cookie, csrf, ensure_origin, set_cookie};
use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

pub(super) fn view(
    app: &App,
    user: Option<&firemage_orm::users::Model>,
    token: Option<&str>,
) -> Value {
    json!({
        "user": user.map(|user| json!({"id":user.id,"username":user.username,"admin":user.admin})),
        "csrf_token": token.map(csrf),
        "password_enabled": true,
        "oidc_enabled": app.runtime.config.oidc_issuer.is_some() && app.runtime.config.oidc_client_id.is_some() && app.runtime.config.public_url.is_some()
    })
}

pub(super) async fn session(State(app): State<App>, headers: HeaderMap) -> Result<Json<Value>> {
    if let Some(token) = cookie(&headers, SESSION_COOKIE)
        && let Ok((user, credential)) = firemage_auth::authenticate(&app.runtime.db, token).await
        && credential.kind == "session"
    {
        return Ok(Json(view(&app, Some(&user), Some(token))));
    }
    Ok(Json(view(&app, None, None)))
}

pub(super) async fn login(
    State(app): State<App>,
    headers: HeaderMap,
    Json(input): Json<firemage_wire::Login>,
) -> Result<Response> {
    ensure_origin(&app, &headers)?;
    let user = crate::login::password_user(&app, input).await?;
    // A successful login replaces the browser's previous session.
    if let Some(old) = cookie(&headers, SESSION_COOKIE)
        && let Ok((owner, credential)) = firemage_auth::authenticate(&app.runtime.db, old).await
        && credential.kind == "session"
    {
        firemage_queries::delete_token(&app.runtime.db, &owner.id, &credential.id).await?;
    }
    issued(&app, &user).await
}

pub(super) async fn issued(app: &App, user: &firemage_orm::users::Model) -> Result<Response> {
    firemage_queries::record_activity(
        &app.runtime.db,
        &user.id,
        &user.username,
        "auth.login",
        "browser",
    )
    .await?;
    let issued = crate::login::session(app, &user.id).await?;
    let cookie = set_cookie(
        app,
        SESSION_COOKIE,
        &issued.token,
        issued.expires_at - firemage_queries::now(),
    )?;
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(view(app, Some(user), Some(&issued.token))),
    )
        .into_response())
}

pub(super) async fn logout(identity: Identity, State(app): State<App>) -> Result<Response> {
    identity.ensure_session()?;
    app.record(&identity, "auth.logout", "browser").await?;
    firemage_queries::delete_token(&app.runtime.db, &identity.user.id, &identity.credential.id)
        .await?;
    Ok((
        [(header::SET_COOKIE, set_cookie(&app, SESSION_COOKIE, "", 0)?)],
        Json(view(&app, None, None)),
    )
        .into_response())
}
