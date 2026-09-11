use super::view;
use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use firemage_wire::{
    AssignEgressPolicy, CatalogEgressPolicy, EgressPolicyInput, EgressPolicyUpdate, Vm,
};

pub async fn list(
    identity: Identity,
    State(app): State<App>,
) -> Result<Json<Vec<CatalogEgressPolicy>>> {
    let rows = firemage_queries::egress_policies(
        &app.runtime.db,
        (!identity.user.admin).then_some(identity.user.id.as_str()),
    )
    .await?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        result.push(view::policy(&app, row).await?);
    }
    Ok(Json(result))
}
pub async fn get(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<CatalogEgressPolicy>> {
    Ok(Json(
        view::policy(&app, view::owned_policy(&app, &identity, &id).await?).await?,
    ))
}
pub async fn create(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<EgressPolicyInput>,
) -> Result<Json<CatalogEgressPolicy>> {
    identity.ensure_admin()?;
    input.validate()?;
    let row = app
        .runtime
        .create_egress_policy(&identity.user.id, input)
        .await?;
    app.record(&identity, "egress.policy.create", &row.id)
        .await?;
    Ok(Json(view::policy(&app, row).await?))
}
pub async fn update(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<EgressPolicyUpdate>,
) -> Result<Json<CatalogEgressPolicy>> {
    identity.ensure_admin()?;
    let row = view::owned_policy(&app, &identity, &id).await?;
    let row = app
        .runtime
        .update_egress_policy(&row.owner_id, &id, input)
        .await?;
    app.record(&identity, "egress.policy.update", &id).await?;
    Ok(Json(view::policy(&app, row).await?))
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    let row = view::owned_policy(&app, &identity, &id).await?;
    app.runtime.delete_egress_policy(&row.owner_id, &id).await?;
    app.record(&identity, "egress.policy.delete", &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn assign(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<AssignEgressPolicy>,
) -> Result<Json<Vm>> {
    identity.ensure_admin()?;
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    let result = app
        .runtime
        .assign_egress_policy(&row.owner_id, &id, input.policy_id)
        .await?;
    app.record(&identity, "vm.egress.policy", &id).await?;
    Ok(Json(result))
}
