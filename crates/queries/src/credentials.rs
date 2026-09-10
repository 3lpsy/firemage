use firemage_orm::{credentials, users};
use sea_orm::{Set, TransactionTrait, prelude::*};

pub async fn insert_credential(
    db: &DatabaseConnection,
    user_id: &str,
    hash: String,
    kind: &str,
    name: &str,
    expires_at: i64,
) -> anyhow::Result<credentials::Model> {
    anyhow::ensure!(
        expires_at > crate::now(),
        "expiration must be in the future"
    );
    Ok(credentials::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        user_id: Set(user_id.into()),
        token_hash: Set(hash),
        kind: Set(kind.into()),
        name: Set(name.into()),
        expires_at: Set(expires_at),
    }
    .insert(db)
    .await?)
}
pub async fn authenticate(
    db: &DatabaseConnection,
    hash: &str,
) -> anyhow::Result<Option<(users::Model, credentials::Model)>> {
    let credential = credentials::Entity::find()
        .filter(credentials::Column::TokenHash.eq(hash))
        .filter(credentials::Column::ExpiresAt.gt(crate::now()))
        .one(db)
        .await?;
    let Some(credential) = credential else {
        return Ok(None);
    };
    Ok(users::Entity::find_by_id(&credential.user_id)
        .filter(users::Column::Disabled.eq(false))
        .one(db)
        .await?
        .map(|user| (user, credential)))
}
pub async fn rotate(
    db: &DatabaseConnection,
    old_hash: &str,
    new_hash: String,
    expires_at: i64,
) -> anyhow::Result<()> {
    let transaction = db.begin().await?;
    let update = credentials::Entity::update_many()
        .col_expr(credentials::Column::TokenHash, Expr::value(new_hash))
        .col_expr(credentials::Column::ExpiresAt, Expr::value(expires_at))
        .filter(credentials::Column::TokenHash.eq(old_hash))
        .filter(credentials::Column::Kind.eq("session"))
        .filter(credentials::Column::ExpiresAt.gt(crate::now()))
        .exec(&transaction)
        .await?;
    anyhow::ensure!(
        update.rows_affected == 1,
        "session expired or already refreshed"
    );
    transaction.commit().await?;
    Ok(())
}
pub async fn api_tokens(
    db: &DatabaseConnection,
    owner: &str,
) -> anyhow::Result<Vec<credentials::Model>> {
    Ok(credentials::Entity::find()
        .filter(credentials::Column::UserId.eq(owner))
        .filter(credentials::Column::Kind.eq("api"))
        .all(db)
        .await?)
}
pub async fn delete_token(db: &DatabaseConnection, owner: &str, id: &str) -> anyhow::Result<bool> {
    Ok(credentials::Entity::delete_many()
        .filter(credentials::Column::Id.eq(id))
        .filter(credentials::Column::UserId.eq(owner))
        .exec(db)
        .await?
        .rows_affected
        == 1)
}
