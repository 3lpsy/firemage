use crate::{
    App,
    error::{Error, Result},
    identity::Identity,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use firemage_wire::{ApiToken, CreateApiToken, Token};
pub async fn refresh(identity: Identity, State(app): State<App>) -> Result<Json<Token>> {
    identity.ensure_session()?;
    let token = firemage_auth::new_token("session");
    let expires_at = firemage_queries::now() + app.management.snapshot().session_ttl()?;
    firemage_queries::rotate(
        &app.runtime.db,
        &identity.credential.token_hash,
        firemage_auth::token_hash(&token),
        expires_at,
    )
    .await?;
    app.record(&identity, "session.refresh", &identity.credential.id)
        .await?;
    Ok(Json(Token { token, expires_at }))
}
pub async fn logout(identity: Identity, State(app): State<App>) -> Result<StatusCode> {
    firemage_queries::delete_token(&app.runtime.db, &identity.user.id, &identity.credential.id)
        .await?;
    app.record(&identity, "token.delete", &identity.credential.id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn list(identity: Identity, State(app): State<App>) -> Result<Json<Vec<ApiToken>>> {
    Ok(Json(
        firemage_queries::api_tokens(&app.runtime.db, &identity.user.id)
            .await?
            .into_iter()
            .map(|t| ApiToken {
                id: t.id,
                name: t.name,
                expires_at: t.expires_at,
            })
            .collect(),
    ))
}
pub async fn create(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<CreateApiToken>,
) -> Result<Json<Token>> {
    identity.ensure_session()?;
    firemage_wire::ensure_name(&input.name)?;
    crate::ensure!(
        input.expires_at > firemage_queries::now(),
        "expiration must be a future Unix timestamp"
    );
    let token = firemage_auth::new_token("api");
    firemage_queries::insert_credential(
        &app.runtime.db,
        &identity.user.id,
        firemage_auth::token_hash(&token),
        "api",
        &input.name,
        input.expires_at,
    )
    .await?;
    app.record(&identity, "token.create", &input.name).await?;
    Ok(Json(Token {
        token,
        expires_at: input.expires_at,
    }))
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    if !firemage_queries::delete_token(&app.runtime.db, &identity.user.id, &id).await? {
        return Err(Error(StatusCode::NOT_FOUND, "token not found".into()));
    }
    app.record(&identity, "token.delete", &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
