use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use firemage_wire::{Network, NetworkSpec, VmSpec};
pub async fn list_networks(
    identity: Identity,
    State(app): State<App>,
) -> Result<Json<Vec<Network>>> {
    Ok(Json(
        (if identity.user.admin {
            firemage_queries::all_networks(&app.runtime.db).await?
        } else {
            firemage_queries::networks(&app.runtime.db, &identity.user.id).await?
        })
        .into_iter()
        .map(|n| serde_json::from_str(&n.spec).map(|spec| Network { id: n.id, spec }))
        .collect::<std::result::Result<_, _>>()
        .map_err(anyhow::Error::from)?,
    ))
}
pub async fn create_network(
    identity: Identity,
    State(app): State<App>,
    Json(spec): Json<NetworkSpec>,
) -> Result<Json<Network>> {
    identity.ensure_admin()?;
    spec.validate()?;
    let _guard = app.runtime.lock("networks").await;
    let row = firemage_queries::insert_network(
        &app.runtime.db,
        &identity.user.id,
        &spec.name,
        serde_json::to_string(&spec).map_err(anyhow::Error::from)?,
    )
    .await?;
    app.record(&identity, "network.create", &spec.name).await?;
    Ok(Json(Network { id: row.id, spec }))
}
pub async fn delete_network(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    let _guard = app.runtime.lock("networks").await;
    let row = owned_network(&app, &identity, &name).await?;
    ensure_unused(&app, &row.owner_id, &row.id).await?;
    firemage_queries::delete_network(&app.runtime.db, &row.owner_id, &row.id).await?;
    app.record(&identity, "network.delete", &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_network(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
    Json(spec): Json<NetworkSpec>,
) -> Result<Json<Network>> {
    identity.ensure_admin()?;
    spec.validate()?;
    let _guard = app.runtime.lock("networks").await;
    let row = owned_network(&app, &identity, &name).await?;
    let mut previous: NetworkSpec = serde_json::from_str(&row.spec).map_err(anyhow::Error::from)?;
    previous.name.clone_from(&spec.name);
    if serde_json::to_value(&previous).map_err(anyhow::Error::from)?
        != serde_json::to_value(&spec).map_err(anyhow::Error::from)?
    {
        ensure_unused(&app, &row.owner_id, &row.id).await?;
    }
    let id = row.id.clone();
    firemage_queries::update_network(
        &app.runtime.db,
        row,
        serde_json::to_string(&spec).map_err(anyhow::Error::from)?,
    )
    .await?;
    app.record(&identity, "network.update", &name).await?;
    Ok(Json(Network { id, spec }))
}
async fn owned_network(
    app: &App,
    identity: &Identity,
    name: &str,
) -> anyhow::Result<firemage_orm::networks::Model> {
    if identity.user.admin {
        let rows = firemage_queries::all_networks(&app.runtime.db).await?;
        rows.iter()
            .find(|row| row.id == name)
            .or_else(|| rows.iter().find(|row| row.name == name))
            .cloned()
            .ok_or_else(|| firemage_queries::NotFound("network").into())
    } else {
        firemage_queries::network(&app.runtime.db, &identity.user.id, name).await
    }
}
async fn ensure_unused(app: &App, owner: &str, name: &str) -> anyhow::Result<()> {
    for vm in firemage_queries::vms(&app.runtime.db, Some(owner)).await? {
        let spec: VmSpec = serde_json::from_str(&vm.spec)?;
        anyhow::ensure!(
            !spec.network.is_some_and(|n| n.network == name),
            "network is referenced by a VM"
        );
    }
    Ok(())
}
