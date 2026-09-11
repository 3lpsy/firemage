use firemage_orm::{activity, credentials, file_assets, networks, users, vms};
use sea_orm::{IntoActiveModel, QueryOrder, QuerySelect, Set, TransactionTrait, prelude::*};

pub async fn update_user(
    db: &DatabaseConnection,
    user: users::Model,
) -> anyhow::Result<users::Model> {
    let tx = db.begin().await?;
    let old = users::Entity::find_by_id(&user.id)
        .one(&tx)
        .await?
        .ok_or(crate::NotFound("user"))?;
    if old.admin && !old.disabled && (!user.admin || user.disabled) {
        anyhow::ensure!(
            users::Entity::find()
                .filter(users::Column::Admin.eq(true))
                .filter(users::Column::Disabled.eq(false))
                .count(&tx)
                .await?
                > 1,
            "cannot remove the last enabled administrator"
        );
    }
    let revoke = old.password_hash != user.password_hash
        || old.admin != user.admin
        || old.disabled != user.disabled
        || old.oidc_subject != user.oidc_subject
        || old.oidc_issuer != user.oidc_issuer;
    let mut active = user.clone().into_active_model();
    active.username = Set(user.username);
    active.password_hash = Set(user.password_hash);
    active.admin = Set(user.admin);
    active.disabled = Set(user.disabled);
    active.oidc_subject = Set(user.oidc_subject);
    active.oidc_issuer = Set(user.oidc_issuer);
    let updated = active.update(&tx).await?;
    if revoke {
        credentials::Entity::delete_many()
            .filter(credentials::Column::UserId.eq(&updated.id))
            .exec(&tx)
            .await?;
    }
    tx.commit().await?;
    Ok(updated)
}
pub async fn user(db: &DatabaseConnection, id: &str) -> anyhow::Result<users::Model> {
    users::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("user").into())
}
pub async fn remove_user(db: &DatabaseConnection, id: &str) -> anyhow::Result<()> {
    let tx = db.begin().await?;
    let user = users::Entity::find_by_id(id)
        .one(&tx)
        .await?
        .ok_or(crate::NotFound("user"))?;
    if user.admin && !user.disabled {
        anyhow::ensure!(
            users::Entity::find()
                .filter(users::Column::Admin.eq(true))
                .filter(users::Column::Disabled.eq(false))
                .count(&tx)
                .await?
                > 1,
            "cannot remove the last enabled administrator"
        );
    }
    anyhow::ensure!(
        vms::Entity::find()
            .filter(vms::Column::OwnerId.eq(id))
            .count(&tx)
            .await?
            == 0
            && networks::Entity::find()
                .filter(networks::Column::OwnerId.eq(id))
                .count(&tx)
                .await?
                == 0,
        "remove the user's VMs and networks before deleting the user"
    );
    anyhow::ensure!(
        file_assets::Entity::find()
            .filter(file_assets::Column::OwnerId.eq(id))
            .count(&tx)
            .await?
            == 0,
        "remove the user's uploaded assets before deleting the user"
    );
    credentials::Entity::delete_many()
        .filter(credentials::Column::UserId.eq(id))
        .exec(&tx)
        .await?;
    users::Entity::delete_by_id(id).exec(&tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn record_activity(
    db: &DatabaseConnection,
    actor_id: &str,
    actor: &str,
    action: &str,
    resource: &str,
) -> anyhow::Result<()> {
    activity::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        at: Set(crate::now()),
        actor_id: Set(actor_id.into()),
        actor: Set(actor.into()),
        action: Set(action.into()),
        resource: Set(resource.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}
pub async fn activity(
    db: &DatabaseConnection,
    actor_id: Option<&str>,
) -> anyhow::Result<Vec<activity::Model>> {
    let mut query = activity::Entity::find();
    if let Some(actor_id) = actor_id {
        query = query.filter(activity::Column::ActorId.eq(actor_id));
    }
    Ok(query
        .order_by_desc(activity::Column::At)
        .order_by_desc(activity::Column::Id)
        .limit(200)
        .all(db)
        .await?)
}
pub async fn vm_by_id(db: &DatabaseConnection, id: &str) -> anyhow::Result<vms::Model> {
    vms::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("VM").into())
}
pub async fn update_vm_spec(
    db: &DatabaseConnection,
    row: vms::Model,
    name: String,
    spec: String,
) -> anyhow::Result<vms::Model> {
    let tx = db.begin().await?;
    let mut active = row.into_active_model();
    active.name = Set(name);
    active.spec = Set(spec);
    active.state = Set("defined".into());
    active.error = Set(None);
    let row = active.update(&tx).await?;
    crate::egress_catalog::bind_vm_egress(&tx, &row).await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn all_networks(db: &DatabaseConnection) -> anyhow::Result<Vec<networks::Model>> {
    Ok(networks::Entity::find().all(db).await?)
}
pub async fn update_network(
    db: &DatabaseConnection,
    row: networks::Model,
    spec: String,
) -> anyhow::Result<()> {
    let mut active = row.into_active_model();
    active.spec = Set(spec);
    active.update(db).await?;
    Ok(())
}
