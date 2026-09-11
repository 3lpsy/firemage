use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SuggestQuery {
    vm: Option<String>,
}

pub async fn suggestion(
    identity: Identity,
    State(app): State<App>,
    Path(name): Path<String>,
    Query(query): Query<SuggestQuery>,
) -> Result<Json<firemage_wire::NetworkAttachment>> {
    let owner = if let Some(id) = &query.vm {
        crate::resources::owned_vm(&app, &identity, id)
            .await?
            .owner_id
    } else {
        identity.user.id.clone()
    };
    Ok(Json(
        app.runtime
            .suggest_network_address(&owner, &name, query.vm.as_deref())
            .await?,
    ))
}
