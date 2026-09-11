use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuplicateVm {
    name: String,
}

pub async fn duplicate(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<DuplicateVm>,
) -> Result<Json<firemage_wire::Vm>> {
    identity.ensure_admin()?;
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    let vm = app
        .runtime
        .duplicate_vm(&row.owner_id, &id, &input.name)
        .await?;
    app.record(&identity, "vm.duplicate", &vm.id).await?;
    Ok(Json(vm))
}
