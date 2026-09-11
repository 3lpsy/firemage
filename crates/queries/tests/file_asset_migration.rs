use firemage_migrations::{Migrator, MigratorTrait};
use firemage_orm::file_assets;
use sea_orm::{
    ConnectionTrait, Database, EntityTrait,
    sea_query::{Alias, Query},
};

#[tokio::test]
async fn storage_name_migration_preserves_existing_assets() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, Some(13)).await.unwrap();
    let owner = firemage_queries::add_user(&db, "admin".into(), None, true, None)
        .await
        .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let insert = Query::insert()
        .into_table(Alias::new("file_assets"))
        .columns(
            [
                "id",
                "owner_id",
                "alias",
                "filename",
                "size_bytes",
                "sha256",
                "created_at",
            ]
            .map(Alias::new),
        )
        .values_panic([
            id.clone().into(),
            owner.id.into(),
            "existing".into(),
            "config.json".into(),
            7i64.into(),
            "0".repeat(64).into(),
            123i64.into(),
        ])
        .to_owned();
    db.execute(db.get_database_backend().build(&insert))
        .await
        .unwrap();
    Migrator::up(&db, None).await.unwrap();
    let asset = file_assets::Entity::find_by_id(&id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(asset.alias, "existing");
    assert_eq!(asset.filename, "config.json");
    assert_eq!(asset.size_bytes, 7);
    assert_eq!(asset.created_at, 123);
    assert_eq!(asset.storage_name, None);
    assert_eq!(
        firemage_queries::all_file_asset_storage_names(&db)
            .await
            .unwrap(),
        vec![id]
    );
}
