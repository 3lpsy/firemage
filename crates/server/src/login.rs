use crate::{
    App,
    error::{Error, Result},
};
use axum::{Json, extract::State, http::StatusCode};

pub(crate) async fn budget(app: &App) -> Result<()> {
    let mut budget = app.login_budget.lock().await;
    if budget.0.elapsed().as_secs() >= 60 {
        *budget = (std::time::Instant::now(), 0);
    }
    if budget.1 >= 30 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "login rate limit reached; retry in a minute".into(),
        ));
    }
    budget.1 += 1;
    Ok(())
}
pub async fn login(
    State(app): State<App>,
    Json(input): Json<firemage_wire::Login>,
) -> Result<Json<firemage_wire::Token>> {
    let user = password_user(&app, input).await?;
    Ok(Json(session(&app, &user.id).await?))
}
pub(crate) async fn password_user(
    app: &App,
    input: firemage_wire::Login,
) -> Result<firemage_orm::users::Model> {
    budget(app).await?;
    firemage_wire::ensure_name(&input.username)?;
    let user = firemage_queries::user_by_name(&app.runtime.db, &input.username).await?;
    let hash = user
        .as_ref()
        .and_then(|u| u.password_hash.clone())
        .unwrap_or_else(|| (*app.dummy_hash).clone());
    let valid = tokio::task::spawn_blocking(move || {
        firemage_auth::is_password_valid(&input.password, &hash)
    })
    .await
    .map_err(anyhow::Error::from)?;
    let user = user
        .filter(|u| valid && u.password_hash.is_some() && !u.disabled)
        .ok_or_else(|| {
            Error(
                StatusCode::UNAUTHORIZED,
                "invalid username or password".into(),
            )
        })?;
    Ok(user)
}
pub(crate) async fn session(app: &App, user_id: &str) -> anyhow::Result<firemage_wire::Token> {
    let token = firemage_auth::new_token("session");
    let expires_at = firemage_queries::now() + app.management.snapshot().session_ttl()?;
    firemage_queries::insert_credential(
        &app.runtime.db,
        user_id,
        firemage_auth::token_hash(&token),
        "session",
        "login",
        expires_at,
    )
    .await?;
    Ok(firemage_wire::Token { token, expires_at })
}
