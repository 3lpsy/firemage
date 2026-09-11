use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use firemage_wire::{TerminalInput, TerminalStatus};

pub async fn status(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<TerminalStatus>> {
    let owner = crate::resources::owned_vm(&app, &identity, &id)
        .await?
        .owner_id;
    Ok(Json(app.runtime.terminal_status(&owner, &id).await?))
}

pub async fn input(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<TerminalInput>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    let owner = crate::resources::owned_vm(&app, &identity, &id)
        .await?
        .owner_id;
    app.runtime.terminal_input(&owner, &id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}
