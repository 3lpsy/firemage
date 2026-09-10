use crate::{App, error::Result, identity::Identity};
use axum::{
    Json,
    extract::{Path, State},
    http::header,
};
use serde_json::{Value, json};

pub async fn ca(
    _identity: Identity,
    State(app): State<App>,
) -> Result<impl axum::response::IntoResponse> {
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-pem-file"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=firemage-ca.pem",
            ),
        ],
        app.runtime.egress().await?.ca_certificate_pem(),
    ))
}
pub async fn status(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    let spec: firemage_wire::VmSpec =
        serde_json::from_str(&row.spec).map_err(anyhow::Error::from)?;
    let policy = app.runtime.effective_egress(&spec);
    let gateway = if let Some(net) = &spec.network {
        let network =
            firemage_queries::network(&app.runtime.db, &row.owner_id, &net.network).await?;
        Some(
            serde_json::from_str::<firemage_wire::NetworkSpec>(&network.spec)
                .map_err(anyhow::Error::from)?
                .gateway,
        )
    } else {
        None
    };
    let http = policy.as_ref().and_then(|p| p.http.as_ref());
    let proxy_url = gateway
        .zip(http)
        .map(|(gateway, http)| format!("http://{gateway}:{}", http.port));
    let tunnels = policy.as_ref().map(|policy| policy.tunnels.iter().map(|t| json!({"name":t.name,"listen_port":t.listen_port,"target_host":t.target_host,"target_port":t.target_port,"endpoint":gateway.map(|ip|format!("{ip}:{}",t.listen_port))})).collect::<Vec<_>>()).unwrap_or_default();
    let fingerprint = if http.is_some() {
        Some(app.runtime.egress().await?.ca_fingerprint())
    } else {
        None
    };
    Ok(Json(
        json!({"enabled":policy.is_some(),"active":app.runtime.is_egress_active(&id).await,"gateway":gateway,"proxy_url":proxy_url,"http_rules":http.map(|h| &h.rules),"tunnels":tunnels,"upstream":policy.as_ref().and_then(|p|p.upstream.as_ref()),"ca_url":"/v1/egress/ca","ca_fingerprint":fingerprint}),
    ))
}
