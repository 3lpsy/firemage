use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use firemage_wire::{CreateUser, UpdateUser, User};

pub fn view(user: firemage_orm::users::Model) -> User {
    User {
        id: user.id,
        username: user.username,
        admin: user.admin,
        disabled: user.disabled,
        oidc_subject: user.oidc_subject,
        oidc_issuer: user.oidc_issuer,
    }
}
fn issuer(app: &App, subject: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        !subject.is_empty() && subject.len() <= 255 && !subject.chars().any(char::is_control),
        "invalid OIDC subject"
    );
    app.runtime
        .config
        .oidc_issuer
        .clone()
        .ok_or_else(|| anyhow::anyhow!("configure OIDC issuer before linking users"))
}
pub async fn create(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<CreateUser>,
) -> Result<Json<User>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    firemage_wire::ensure_name(&input.username)?;
    crate::ensure!(
        input.password.is_some() || input.oidc_subject.is_some(),
        "password or OIDC subject required"
    );
    let issuer = input
        .oidc_subject
        .as_deref()
        .map(|subject| issuer(&app, subject))
        .transpose()?;
    let hash = tokio::task::spawn_blocking(move || {
        input
            .password
            .as_deref()
            .map(firemage_auth::hash_password)
            .transpose()
    })
    .await
    .map_err(anyhow::Error::from)??;
    let user = firemage_queries::add_user_with_issuer(
        &app.runtime.db,
        input.username,
        hash,
        input.admin,
        input.oidc_subject,
        issuer,
    )
    .await?;
    app.record(&identity, "user.create", &user.id).await?;
    Ok(Json(view(user)))
}
pub async fn list(identity: Identity, State(app): State<App>) -> Result<Json<Vec<User>>> {
    identity.ensure_admin()?;
    Ok(Json(
        firemage_queries::users(&app.runtime.db)
            .await?
            .into_iter()
            .map(view)
            .collect(),
    ))
}
pub async fn update(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<UpdateUser>,
) -> Result<Json<User>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    firemage_wire::ensure_name(&input.username)?;
    let _guard = app.runtime.lock("users").await;
    let mut user = firemage_queries::user(&app.runtime.db, &id).await?;
    user.username = input.username;
    user.admin = input.admin;
    user.disabled = input.disabled;
    if let Some(password) = input.password {
        user.password_hash = Some(
            tokio::task::spawn_blocking(move || firemage_auth::hash_password(&password))
                .await
                .map_err(anyhow::Error::from)??,
        );
    }
    crate::ensure!(
        !input.clear_oidc || input.oidc_subject.is_none(),
        "choose an OIDC subject or clear the binding"
    );
    if input.clear_oidc {
        user.oidc_subject = None;
        user.oidc_issuer = None;
    }
    if let Some(subject) = input.oidc_subject {
        user.oidc_issuer = Some(issuer(&app, &subject)?);
        user.oidc_subject = Some(subject);
    }
    crate::ensure!(
        user.password_hash.is_some() || user.oidc_subject.is_some(),
        "user needs a password or OIDC binding"
    );
    let user = firemage_queries::update_user(&app.runtime.db, user).await?;
    app.record(&identity, "user.update", &id).await?;
    Ok(Json(view(user)))
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    let _guard = app.runtime.lock("users").await;
    firemage_queries::remove_user(&app.runtime.db, &id).await?;
    app.record(&identity, "user.delete", &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
