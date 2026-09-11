use crate::{App, error::Result, identity::Identity};
use firemage_orm::{egress_policies, upstream_proxies, vms};
use firemage_wire::{
    CatalogEgressPolicy, CatalogUpstreamProxy, EgressPolicyInput, EgressPolicyReference,
    EgressVmReference, UpstreamProxyInput,
};

fn vm(row: vms::Model) -> EgressVmReference {
    EgressVmReference {
        id: row.id,
        owner_id: row.owner_id,
        name: row.name,
        state: row.state,
        error: row.error,
    }
}
pub async fn owned_policy(
    app: &App,
    identity: &Identity,
    id: &str,
) -> Result<egress_policies::Model> {
    Ok(if identity.user.admin {
        firemage_queries::egress_policy_by_id(&app.runtime.db, id).await?
    } else {
        firemage_queries::egress_policy(&app.runtime.db, &identity.user.id, id).await?
    })
}
pub async fn owned_proxy(
    app: &App,
    identity: &Identity,
    id: &str,
) -> Result<upstream_proxies::Model> {
    Ok(if identity.user.admin {
        firemage_queries::upstream_proxy_by_id(&app.runtime.db, id).await?
    } else {
        firemage_queries::upstream_proxy(&app.runtime.db, &identity.user.id, id).await?
    })
}
pub async fn policy(app: &App, row: egress_policies::Model) -> Result<CatalogEgressPolicy> {
    let vms: Vec<_> = firemage_queries::policy_vms(&app.runtime.db, &row.owner_id, &row.id)
        .await?
        .into_iter()
        .map(vm)
        .collect();
    Ok(CatalogEgressPolicy {
        id: row.id,
        owner_id: row.owner_id,
        revision: u64::try_from(row.revision).map_err(anyhow::Error::from)?,
        input: EgressPolicyInput {
            alias: row.alias,
            policy: serde_json::from_str(&row.spec).map_err(anyhow::Error::from)?,
            upstream_proxy_id: row.upstream_proxy_id,
        },
        vm_count: vms.len(),
        vms,
    })
}
pub async fn proxy(app: &App, row: upstream_proxies::Model) -> Result<CatalogUpstreamProxy> {
    let policies: Vec<_> =
        firemage_queries::proxy_policies(&app.runtime.db, &row.owner_id, &row.id)
            .await?
            .into_iter()
            .map(|p| EgressPolicyReference {
                id: p.id,
                alias: p.alias,
            })
            .collect();
    let vms: Vec<_> = firemage_queries::proxy_vms(&app.runtime.db, &row.owner_id, &row.id)
        .await?
        .into_iter()
        .map(vm)
        .collect();
    Ok(CatalogUpstreamProxy {
        id: row.id,
        owner_id: row.owner_id,
        revision: u64::try_from(row.revision).map_err(anyhow::Error::from)?,
        input: UpstreamProxyInput {
            alias: row.alias,
            proxy: serde_json::from_str(&row.spec).map_err(anyhow::Error::from)?,
            ca_secret: row.ca_secret,
        },
        vm_count: vms.len(),
        vms,
        policy_count: policies.len(),
        policies,
    })
}
