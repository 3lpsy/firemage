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
use firemage_wire::{PutSecret, SecretMetadata};

pub async fn list(identity: Identity, State(app): State<App>) -> Result<Json<Vec<SecretMetadata>>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    Ok(Json(
        app.runtime.secrets().await?.list(&identity.user.id).await?,
    ))
}
pub async fn put(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
    Json(input): Json<PutSecret>,
) -> Result<Json<SecretMetadata>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    let metadata = app
        .runtime
        .secrets()
        .await?
        .put(&identity.user.id, &name, &input.value)
        .await?;
    app.record(&identity, "secret.put", &name).await?;
    Ok(Json(metadata))
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    if !app
        .runtime
        .secrets()
        .await?
        .delete(&identity.user.id, &name)
        .await?
    {
        return Err(Error(StatusCode::NOT_FOUND, "secret not found".into()));
    }
    app.record(&identity, "secret.delete", &name).await?;
    Ok(StatusCode::NO_CONTENT)
}
