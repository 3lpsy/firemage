use super::validation;
use firemage_orm::{egress_policies, vm_egress_policies, vms};
use sea_orm::{ConnectionTrait, QueryOrder, Set, TransactionTrait, prelude::*};

pub async fn policy_vms(
    db: &impl ConnectionTrait,
    owner: &str,
    id: &str,
) -> anyhow::Result<Vec<vms::Model>> {
    super::egress_policy(db, owner, id).await?;
    let bindings = vm_egress_policies::Entity::find()
        .filter(vm_egress_policies::Column::OwnerId.eq(owner))
        .filter(vm_egress_policies::Column::PolicyId.eq(id))
        .all(db)
        .await?;
    Ok(vms::Entity::find()
        .filter(vms::Column::OwnerId.eq(owner))
        .filter(vms::Column::Id.is_in(bindings.into_iter().map(|b| b.vm_id)))
        .order_by_asc(vms::Column::Name)
        .order_by_asc(vms::Column::Id)
        .all(db)
        .await?)
}
pub async fn proxy_policies(
    db: &impl ConnectionTrait,
    owner: &str,
    id: &str,
) -> anyhow::Result<Vec<egress_policies::Model>> {
    super::upstream_proxy(db, owner, id).await?;
    Ok(egress_policies::Entity::find()
        .filter(egress_policies::Column::OwnerId.eq(owner))
        .filter(egress_policies::Column::UpstreamProxyId.eq(id))
        .order_by_asc(egress_policies::Column::Alias)
        .all(db)
        .await?)
}
pub async fn proxy_vms(
    db: &impl ConnectionTrait,
    owner: &str,
    id: &str,
) -> anyhow::Result<Vec<vms::Model>> {
    let policies = proxy_policies(db, owner, id).await?;
    let bindings = vm_egress_policies::Entity::find()
        .filter(vm_egress_policies::Column::OwnerId.eq(owner))
        .filter(vm_egress_policies::Column::PolicyId.is_in(policies.into_iter().map(|p| p.id)))
        .all(db)
        .await?;
    Ok(vms::Entity::find()
        .filter(vms::Column::OwnerId.eq(owner))
        .filter(vms::Column::Id.is_in(bindings.into_iter().map(|b| b.vm_id)))
        .order_by_asc(vms::Column::Name)
        .order_by_asc(vms::Column::Id)
        .all(db)
        .await?)
}

/// Keep the restrictive catalog reference in the same transaction as the VM spec.
pub(crate) async fn bind_vm_egress(
    db: &impl ConnectionTrait,
    row: &vms::Model,
) -> anyhow::Result<()> {
    validation::ids(&row.owner_id, &row.id)?;
    let spec: serde_json::Value = serde_json::from_str(&row.spec)?;
    let policy = spec.get("egress_policy").filter(|v| !v.is_null());
    let policy = policy
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("egress policy must be a UUID"))
        })
        .transpose()?;
    if let Some(id) = policy {
        anyhow::ensure!(
            spec.get("egress").is_none_or(serde_json::Value::is_null),
            "choose a catalog policy or inline egress, not both"
        );
        super::egress_policy(db, &row.owner_id, id).await?;
    }
    vm_egress_policies::Entity::delete_by_id(&row.id)
        .exec(db)
        .await?;
    if let Some(policy) = policy {
        vm_egress_policies::ActiveModel {
            vm_id: Set(row.id.clone()),
            owner_id: Set(row.owner_id.clone()),
            policy_id: Set(policy.into()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub async fn set_vm_egress_policy(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
    policy_id: Option<&str>,
    http_port: u16,
) -> anyhow::Result<vms::Model> {
    validation::ids(owner, id)?;
    if let Some(policy) = policy_id {
        firemage_wire::ensure_asset_id(policy)?;
    }
    anyhow::ensure!(http_port >= 1024, "egress HTTP port must be at least 1024");
    let tx = db.begin().await?;
    let row = vms::Entity::find_by_id(id)
        .filter(vms::Column::OwnerId.eq(owner))
        .one(&tx)
        .await?
        .ok_or(crate::NotFound("VM"))?;
    let mut spec: serde_json::Value = serde_json::from_str(&row.spec)?;
    anyhow::ensure!(spec.is_object(), "invalid VM specification");
    spec["egress_policy"] = serde_json::to_value(policy_id)?;
    spec["egress_http_port"] = http_port.into();
    spec["egress"] = serde_json::Value::Null;
    let mut active: vms::ActiveModel = row.into();
    active.spec = Set(serde_json::to_string(&spec)?);
    let row = active.update(&tx).await?;
    bind_vm_egress(&tx, &row).await?;
    tx.commit().await?;
    Ok(row)
}
