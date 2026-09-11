use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
};
use firemage_wire::{Vm, VmConfigImport, VmConfigPreview};
use serde_json::{Value, json};

pub async fn export(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    Ok(Json(
        json!({"toml": app.runtime.export_vm_config(&row.owner_id, &id).await?}),
    ))
}
pub async fn preview(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<VmConfigImport>,
) -> Result<Json<VmConfigPreview>> {
    identity.ensure_admin()?;
    Ok(Json(
        app.runtime
            .resolve_vm_config(&identity.user.id, &input, None)
            .await?
            .1,
    ))
}
pub async fn import(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<VmConfigImport>,
) -> Result<Json<Vm>> {
    identity.ensure_admin()?;
    let (spec, _) = app
        .runtime
        .resolve_vm_config(&identity.user.id, &input, None)
        .await?;
    let vm = app.runtime.define(&identity.user.id, spec).await?;
    app.record(&identity, "vm.import", &vm.id).await?;
    Ok(Json(vm))
}
pub async fn preview_existing(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<VmConfigImport>,
) -> Result<Json<VmConfigPreview>> {
    identity.ensure_admin()?;
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    Ok(Json(
        app.runtime
            .resolve_vm_config(&row.owner_id, &input, Some(&id))
            .await?
            .1,
    ))
}
pub async fn import_existing(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<VmConfigImport>,
) -> Result<Json<Vm>> {
    identity.ensure_admin()?;
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    let (spec, _) = app
        .runtime
        .resolve_vm_config(&row.owner_id, &input, Some(&id))
        .await?;
    let vm = app.runtime.update(&row.owner_id, &id, spec).await?;
    app.record(&identity, "vm.import-config", &id).await?;
    Ok(Json(vm))
}
