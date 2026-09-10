use firemage_migrations::{Migrator, MigratorTrait};
use firemage_orm::users;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, EntityTrait, QueryFilter, Set,
};
pub async fn connect(url: &str) -> anyhow::Result<sea_orm::DatabaseConnection> {
    let mut options = ConnectOptions::new(url);
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await?;
    Migrator::up(&db, None).await?;
    Ok(db)
}
pub async fn user_by_name(
    db: &sea_orm::DatabaseConnection,
    name: &str,
) -> anyhow::Result<Option<users::Model>> {
    Ok(users::Entity::find()
        .filter(users::Column::Username.eq(name))
        .one(db)
        .await?)
}
pub async fn user_by_subject(
    db: &sea_orm::DatabaseConnection,
    sub: &str,
    issuer: &str,
) -> anyhow::Result<Option<users::Model>> {
    Ok(users::Entity::find()
        .filter(users::Column::OidcSubject.eq(sub))
        .filter(users::Column::OidcIssuer.eq(issuer))
        .one(db)
        .await?)
}
pub async fn add_user(
    db: &impl sea_orm::ConnectionTrait,
    username: String,
    password_hash: Option<String>,
    admin: bool,
    oidc_subject: Option<String>,
) -> anyhow::Result<users::Model> {
    add_user_with_issuer(db, username, password_hash, admin, oidc_subject, None).await
}
pub async fn add_user_with_issuer(
    db: &impl sea_orm::ConnectionTrait,
    username: String,
    password_hash: Option<String>,
    admin: bool,
    oidc_subject: Option<String>,
    oidc_issuer: Option<String>,
) -> anyhow::Result<users::Model> {
    Ok(users::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        username: Set(username),
        password_hash: Set(password_hash),
        admin: Set(admin),
        disabled: Set(false),
        oidc_subject: Set(oidc_subject),
        oidc_issuer: Set(oidc_issuer),
    }
    .insert(db)
    .await?)
}
pub async fn users(db: &sea_orm::DatabaseConnection) -> anyhow::Result<Vec<users::Model>> {
    Ok(users::Entity::find().all(db).await?)
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64
}

pub async fn bootstrap(
    db: &sea_orm::DatabaseConnection,
    username: String,
    hash: String,
) -> anyhow::Result<()> {
    use sea_orm::TransactionTrait;
    let transaction = db.begin().await?;
    anyhow::ensure!(
        users::Entity::find().one(&transaction).await?.is_none(),
        "bootstrap requires an empty user database"
    );
    add_user(&transaction, username, Some(hash), true, None).await?;
    transaction.commit().await?;
    Ok(())
}
