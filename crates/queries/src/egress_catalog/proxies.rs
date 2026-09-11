use super::validation;
use firemage_orm::upstream_proxies;
use firemage_wire::{UpstreamProxyInput, UpstreamProxyUpdate};
use sea_orm::{ConnectionTrait, QueryOrder, Set, TransactionTrait, prelude::*};

pub async fn upstream_proxy(
    db: &impl ConnectionTrait,
    owner: &str,
    id: &str,
) -> anyhow::Result<upstream_proxies::Model> {
    validation::ids(owner, id)?;
    upstream_proxies::Entity::find_by_id(id)
        .filter(upstream_proxies::Column::OwnerId.eq(owner))
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("upstream proxy").into())
}
pub async fn upstream_proxy_by_id(
    db: &impl ConnectionTrait,
    id: &str,
) -> anyhow::Result<upstream_proxies::Model> {
    firemage_wire::ensure_asset_id(id)?;
    upstream_proxies::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("upstream proxy").into())
}
pub async fn upstream_proxies(
    db: &impl ConnectionTrait,
    owner: Option<&str>,
) -> anyhow::Result<Vec<upstream_proxies::Model>> {
    let mut query = upstream_proxies::Entity::find();
    if let Some(owner) = owner {
        firemage_wire::ensure_asset_id(owner)?;
        query = query.filter(upstream_proxies::Column::OwnerId.eq(owner));
    }
    Ok(query
        .order_by_asc(upstream_proxies::Column::Alias)
        .order_by_asc(upstream_proxies::Column::Id)
        .all(db)
        .await?)
}
async fn validate(
    db: &impl ConnectionTrait,
    owner: &str,
    input: &UpstreamProxyInput,
) -> anyhow::Result<()> {
    firemage_wire::ensure_asset_id(owner)?;
    input.validate()?;
    let mut names: Vec<_> = input
        .proxy
        .username
        .iter()
        .chain(input.proxy.password.iter())
        .filter_map(|s| s.secret_name())
        .collect();
    names.extend(input.ca_secret.as_deref());
    validation::secrets(db, owner, names).await
}
pub async fn insert_upstream_proxy(
    db: &DatabaseConnection,
    owner: &str,
    input: &UpstreamProxyInput,
) -> anyhow::Result<upstream_proxies::Model> {
    let tx = db.begin().await?;
    validate(&tx, owner, input).await?;
    let row = upstream_proxies::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(owner.into()),
        alias: Set(input.alias.clone()),
        revision: Set(1),
        spec: Set(serde_json::to_string(&input.proxy)?),
        ca_secret: Set(input.ca_secret.clone()),
    }
    .insert(&tx)
    .await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn update_upstream_proxy(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
    input: &UpstreamProxyUpdate,
) -> anyhow::Result<upstream_proxies::Model> {
    validation::ids(owner, id)?;
    let revision = validation::revision(input.revision)?;
    let tx = db.begin().await?;
    upstream_proxy(&tx, owner, id).await?;
    validate(&tx, owner, &input.input).await?;
    let result = upstream_proxies::Entity::update_many()
        .col_expr(
            upstream_proxies::Column::Alias,
            Expr::value(input.input.alias.clone()),
        )
        .col_expr(
            upstream_proxies::Column::Spec,
            Expr::value(serde_json::to_string(&input.input.proxy)?),
        )
        .col_expr(
            upstream_proxies::Column::CaSecret,
            Expr::value(input.input.ca_secret.clone()),
        )
        .col_expr(
            upstream_proxies::Column::Revision,
            Expr::value(revision + 1),
        )
        .filter(upstream_proxies::Column::Id.eq(id))
        .filter(upstream_proxies::Column::OwnerId.eq(owner))
        .filter(upstream_proxies::Column::Revision.eq(revision))
        .exec(&tx)
        .await?;
    if result.rows_affected != 1 {
        return Err(validation::CatalogRevisionConflict.into());
    }
    let row = upstream_proxy(&tx, owner, id).await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn delete_upstream_proxy(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
) -> anyhow::Result<()> {
    let tx = db.begin().await?;
    upstream_proxy(&tx, owner, id).await?;
    if !super::proxy_policies(&tx, owner, id).await?.is_empty() {
        return Err(validation::CatalogInUse.into());
    }
    upstream_proxies::Entity::delete_many()
        .filter(upstream_proxies::Column::Id.eq(id))
        .filter(upstream_proxies::Column::OwnerId.eq(owner))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
