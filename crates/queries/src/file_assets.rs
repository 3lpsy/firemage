use firemage_orm::file_assets;
use sea_orm::{QueryOrder, Set, prelude::*};

pub async fn file_assets(
    db: &DatabaseConnection,
    owner: &str,
) -> anyhow::Result<Vec<file_assets::Model>> {
    firemage_wire::ensure_asset_id(owner)?;
    Ok(file_assets::Entity::find()
        .filter(file_assets::Column::OwnerId.eq(owner))
        .order_by_asc(file_assets::Column::Alias)
        .all(db)
        .await?)
}
pub async fn file_asset(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
) -> anyhow::Result<file_assets::Model> {
    firemage_wire::ensure_asset_id(owner)?;
    firemage_wire::ensure_asset_id(id)?;
    file_assets::Entity::find_by_id(id)
        .filter(file_assets::Column::OwnerId.eq(owner))
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("asset").into())
}
pub async fn insert_file_asset(
    db: &DatabaseConnection,
    row: file_assets::Model,
) -> anyhow::Result<()> {
    firemage_wire::ensure_asset_id(&row.owner_id)?;
    firemage_wire::ensure_asset_id(&row.id)?;
    firemage_wire::FileAssetUpload {
        alias: row.alias.clone(),
        filename: row.filename.clone(),
    }
    .validate()?;
    anyhow::ensure!(
        (0..=firemage_wire::FILE_ASSET_MAX_BYTES as i64).contains(&row.size_bytes)
            && row.sha256.len() == 64
            && row.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid asset metadata"
    );
    file_assets::Entity::insert(file_assets::ActiveModel {
        id: Set(row.id),
        owner_id: Set(row.owner_id),
        alias: Set(row.alias),
        filename: Set(row.filename),
        size_bytes: Set(row.size_bytes),
        sha256: Set(row.sha256),
        created_at: Set(row.created_at),
    })
    .exec(db)
    .await?;
    Ok(())
}
pub async fn alias_file_asset(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
    alias: &str,
) -> anyhow::Result<()> {
    firemage_wire::ensure_asset_alias(alias)?;
    let row = file_asset(db, owner, id).await?;
    let mut active: file_assets::ActiveModel = row.into();
    active.alias = Set(alias.into());
    active.update(db).await?;
    Ok(())
}
pub async fn delete_file_asset(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
) -> anyhow::Result<()> {
    file_asset(db, owner, id).await?;
    file_assets::Entity::delete_many()
        .filter(file_assets::Column::Id.eq(id))
        .filter(file_assets::Column::OwnerId.eq(owner))
        .exec(db)
        .await?;
    Ok(())
}
