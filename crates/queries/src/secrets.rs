use firemage_orm::secrets;
use sea_orm::{QueryOrder, Set, prelude::*, sea_query::OnConflict};

pub async fn put_secret(
    db: &DatabaseConnection,
    owner: &str,
    name: &str,
    ciphertext: String,
) -> anyhow::Result<i64> {
    let updated_at = crate::now();
    secrets::Entity::insert(secrets::ActiveModel {
        owner_id: Set(owner.into()),
        name: Set(name.into()),
        ciphertext: Set(ciphertext),
        updated_at: Set(updated_at),
    })
    .on_conflict(
        OnConflict::columns([secrets::Column::OwnerId, secrets::Column::Name])
            .update_columns([secrets::Column::Ciphertext, secrets::Column::UpdatedAt])
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    Ok(updated_at)
}
pub async fn secret(
    db: &DatabaseConnection,
    owner: &str,
    name: &str,
) -> anyhow::Result<secrets::Model> {
    secrets::Entity::find_by_id((owner.to_owned(), name.to_owned()))
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("secret").into())
}
pub async fn secrets(db: &DatabaseConnection, owner: &str) -> anyhow::Result<Vec<secrets::Model>> {
    Ok(secrets::Entity::find()
        .filter(secrets::Column::OwnerId.eq(owner))
        .order_by_asc(secrets::Column::Name)
        .all(db)
        .await?)
}
pub async fn delete_secret(
    db: &DatabaseConnection,
    owner: &str,
    name: &str,
) -> anyhow::Result<bool> {
    Ok(
        secrets::Entity::delete_by_id((owner.to_owned(), name.to_owned()))
            .exec(db)
            .await?
            .rows_affected
            == 1,
    )
}
