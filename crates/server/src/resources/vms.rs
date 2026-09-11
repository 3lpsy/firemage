use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use firemage_wire::{RawRequest, RawResponse, Vm, VmAction, VmSpec};
use serde::Deserialize;
use serde_json::{Value, json};

pub async fn list_vms(identity: Identity, State(app): State<App>) -> Result<Json<Vec<Vm>>> {
    Ok(Json(
        firemage_queries::vms(
            &app.runtime.db,
            if identity.user.admin {
                None
            } else {
                Some(&identity.user.id)
            },
        )
        .await?
        .into_iter()
        .map(firemage_runtime::view)
        .collect::<anyhow::Result<_>>()?,
    ))
}
pub async fn create_vm(
    identity: Identity,
    State(app): State<App>,
    Json(spec): Json<VmSpec>,
) -> Result<Json<Vm>> {
    identity.ensure_admin()?;
    let vm = app.runtime.define(&identity.user.id, spec).await?;
    app.record(&identity, "vm.create", &vm.id).await?;
    Ok(Json(vm))
}
pub async fn get_vm(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<Vm>> {
    Ok(Json(firemage_runtime::view(
        owned_vm(&app, &identity, &id).await?,
    )?))
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeleteQuery {
    delete_snapshots: bool,
}
pub async fn delete_vm(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    let owner = owned_vm(&app, &identity, &id).await?.owner_id;
    app.runtime
        .delete_with_snapshots(&owner, &id, query.delete_snapshots)
        .await?;
    app.record(&identity, "vm.delete", &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn action(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(action): Json<VmAction>,
) -> Result<Json<Vm>> {
    identity.ensure_admin()?;
    let owner = owned_vm(&app, &identity, &id).await?.owner_id;
    let label = serde_json::to_value(&action).map_err(anyhow::Error::from)?["action"]
        .as_str()
        .unwrap_or("action")
        .to_owned();
    let vm = app.runtime.action(&owner, &id, action).await?;
    app.record(&identity, &format!("vm.{label}"), &id).await?;
    Ok(Json(vm))
}
pub async fn raw(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<RawRequest>,
) -> Result<Json<RawResponse>> {
    identity.ensure_admin()?;
    let _guard = app.runtime.lock(&id).await;
    let row = owned_vm(&app, &identity, &id).await?;
    app.runtime.ensure_raw_request(&row, &input).await?;
    let (status, body) = firemage_firecracker::Firecracker::new(std::path::Path::new(&row.socket))?
        .request(&input.method, &input.path, input.body.as_ref())
        .await?;
    if input.method != "GET" {
        app.runtime.refresh(row).await?;
        app.record(&identity, "vm.firecracker", &id).await?;
    }
    Ok(Json(RawResponse { status, body }))
}
#[derive(Deserialize)]
pub struct FileQuery {
    path: String,
}
pub async fn output(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Result<Json<Value>> {
    let owner = owned_vm(&app, &identity, &id).await?.owner_id;
    let bytes = app.runtime.output(&owner, &id, &query.path).await?;
    use base64::Engine;
    Ok(Json(
        json!({"path":query.path,"base64":base64::engine::general_purpose::STANDARD.encode(bytes)}),
    ))
}
pub async fn update_vm(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(spec): Json<VmSpec>,
) -> Result<Json<Vm>> {
    identity.ensure_admin()?;
    let owner = owned_vm(&app, &identity, &id).await?.owner_id;
    let vm = app.runtime.update(&owner, &id, spec).await?;
    app.record(&identity, "vm.update", &id).await?;
    Ok(Json(vm))
}
pub(crate) async fn owned_vm(
    app: &App,
    identity: &Identity,
    id: &str,
) -> anyhow::Result<firemage_orm::vms::Model> {
    if identity.user.admin {
        firemage_queries::vm_by_id(&app.runtime.db, id).await
    } else {
        firemage_queries::vm(&app.runtime.db, &identity.user.id, id).await
    }
}
