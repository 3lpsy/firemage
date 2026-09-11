use super::validation;
use firemage_orm::egress_policies;
use firemage_wire::{EgressPolicyInput, EgressPolicyUpdate};
use sea_orm::{ConnectionTrait, QueryOrder, Set, TransactionTrait, prelude::*};

pub async fn egress_policy(
    db: &impl ConnectionTrait,
    owner: &str,
    id: &str,
) -> anyhow::Result<egress_policies::Model> {
    validation::ids(owner, id)?;
    egress_policies::Entity::find_by_id(id)
        .filter(egress_policies::Column::OwnerId.eq(owner))
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("egress policy").into())
}
pub async fn egress_policy_by_id(
    db: &impl ConnectionTrait,
    id: &str,
) -> anyhow::Result<egress_policies::Model> {
    firemage_wire::ensure_asset_id(id)?;
    egress_policies::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("egress policy").into())
}
pub async fn egress_policies(
    db: &impl ConnectionTrait,
    owner: Option<&str>,
) -> anyhow::Result<Vec<egress_policies::Model>> {
    let mut query = egress_policies::Entity::find();
    if let Some(owner) = owner {
        firemage_wire::ensure_asset_id(owner)?;
        query = query.filter(egress_policies::Column::OwnerId.eq(owner));
    }
    Ok(query
        .order_by_asc(egress_policies::Column::Alias)
        .order_by_asc(egress_policies::Column::Id)
        .all(db)
        .await?)
}
async fn validate(
    db: &impl ConnectionTrait,
    owner: &str,
    input: &EgressPolicyInput,
) -> anyhow::Result<()> {
    firemage_wire::ensure_asset_id(owner)?;
    input.validate()?;
    if let Some(id) = &input.upstream_proxy_id {
        super::upstream_proxy(db, owner, id).await?;
    }
    validation::secrets(db, owner, input.policy.secret_names()).await
}
pub async fn insert_egress_policy(
    db: &DatabaseConnection,
    owner: &str,
    input: &EgressPolicyInput,
) -> anyhow::Result<egress_policies::Model> {
    let tx = db.begin().await?;
    validate(&tx, owner, input).await?;
    let row = egress_policies::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(owner.into()),
        alias: Set(input.alias.clone()),
        revision: Set(1),
        spec: Set(serde_json::to_string(&input.policy)?),
        upstream_proxy_id: Set(input.upstream_proxy_id.clone()),
    }
    .insert(&tx)
    .await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn update_egress_policy(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
    input: &EgressPolicyUpdate,
) -> anyhow::Result<egress_policies::Model> {
    validation::ids(owner, id)?;
    let revision = validation::revision(input.revision)?;
    let tx = db.begin().await?;
    egress_policy(&tx, owner, id).await?;
    validate(&tx, owner, &input.input).await?;
    let result = egress_policies::Entity::update_many()
        .col_expr(
            egress_policies::Column::Alias,
            Expr::value(input.input.alias.clone()),
        )
        .col_expr(
            egress_policies::Column::Spec,
            Expr::value(serde_json::to_string(&input.input.policy)?),
        )
        .col_expr(
            egress_policies::Column::UpstreamProxyId,
            Expr::value(input.input.upstream_proxy_id.clone()),
        )
        .col_expr(egress_policies::Column::Revision, Expr::value(revision + 1))
        .filter(egress_policies::Column::Id.eq(id))
        .filter(egress_policies::Column::OwnerId.eq(owner))
        .filter(egress_policies::Column::Revision.eq(revision))
        .exec(&tx)
        .await?;
    if result.rows_affected != 1 {
        return Err(validation::CatalogRevisionConflict.into());
    }
    let row = egress_policy(&tx, owner, id).await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn delete_egress_policy(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
) -> anyhow::Result<()> {
    let tx = db.begin().await?;
    egress_policy(&tx, owner, id).await?;
    if !super::policy_vms(&tx, owner, id).await?.is_empty() {
        return Err(validation::CatalogInUse.into());
    }
    egress_policies::Entity::delete_many()
        .filter(egress_policies::Column::Id.eq(id))
        .filter(egress_policies::Column::OwnerId.eq(owner))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
