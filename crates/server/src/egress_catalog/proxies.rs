use super::view;
use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use firemage_wire::{CatalogUpstreamProxy, UpstreamProxyInput, UpstreamProxyUpdate};

pub async fn list(
    identity: Identity,
    State(app): State<App>,
) -> Result<Json<Vec<CatalogUpstreamProxy>>> {
    let rows = firemage_queries::upstream_proxies(
        &app.runtime.db,
        (!identity.user.admin).then_some(identity.user.id.as_str()),
    )
    .await?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        result.push(view::proxy(&app, row).await?);
    }
    Ok(Json(result))
}
pub async fn get(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<CatalogUpstreamProxy>> {
    Ok(Json(
        view::proxy(&app, view::owned_proxy(&app, &identity, &id).await?).await?,
    ))
}
pub async fn create(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<UpstreamProxyInput>,
) -> Result<Json<CatalogUpstreamProxy>> {
    identity.ensure_admin()?;
    input.validate()?;
    let row = app
        .runtime
        .create_upstream_proxy(&identity.user.id, input)
        .await?;
    app.record(&identity, "egress.proxy.create", &row.id)
        .await?;
    Ok(Json(view::proxy(&app, row).await?))
}
pub async fn update(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    Json(input): Json<UpstreamProxyUpdate>,
) -> Result<Json<CatalogUpstreamProxy>> {
    identity.ensure_admin()?;
    let row = view::owned_proxy(&app, &identity, &id).await?;
    let row = app
        .runtime
        .update_upstream_proxy(&row.owner_id, &id, input)
        .await?;
    app.record(&identity, "egress.proxy.update", &id).await?;
    Ok(Json(view::proxy(&app, row).await?))
}
pub async fn delete(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    identity.ensure_admin()?;
    let row = view::owned_proxy(&app, &identity, &id).await?;
    app.runtime
        .delete_upstream_proxy(&row.owner_id, &id)
        .await?;
    app.record(&identity, "egress.proxy.delete", &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
