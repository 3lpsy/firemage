use crate::DatabaseConnection;
use firemage_orm::snapshots;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

pub async fn all_snapshots(db: &DatabaseConnection) -> anyhow::Result<Vec<snapshots::Model>> {
    Ok(snapshots::Entity::find()
        .order_by_desc(snapshots::Column::CreatedAt)
        .all(db)
        .await?)
}
pub async fn snapshot_by_id(db: &DatabaseConnection, id: &str) -> anyhow::Result<snapshots::Model> {
    firemage_wire::ensure_asset_id(id)?;
    snapshots::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("snapshot").into())
}

pub async fn snapshots(
    db: &DatabaseConnection,
    owner: &str,
) -> anyhow::Result<Vec<snapshots::Model>> {
    firemage_wire::ensure_asset_id(owner)?;
    Ok(snapshots::Entity::find()
        .filter(snapshots::Column::OwnerId.eq(owner))
        .order_by_desc(snapshots::Column::CreatedAt)
        .all(db)
        .await?)
}
pub async fn snapshot(
    db: &DatabaseConnection,
    owner: &str,
    id: &str,
) -> anyhow::Result<snapshots::Model> {
    firemage_wire::ensure_asset_id(owner)?;
    firemage_wire::ensure_asset_id(id)?;
    snapshots::Entity::find_by_id(id)
        .filter(snapshots::Column::OwnerId.eq(owner))
        .one(db)
        .await?
        .ok_or_else(|| crate::NotFound("snapshot").into())
}
pub async fn insert_snapshot(db: &DatabaseConnection, row: snapshots::Model) -> anyhow::Result<()> {
    firemage_wire::ensure_asset_id(&row.id)?;
    firemage_wire::ensure_asset_id(&row.owner_id)?;
    firemage_wire::ensure_asset_alias(&row.alias)?;
    firemage_wire::ensure_name(&row.source_vm_name)?;
    if let Some(id) = &row.source_vm_id {
        firemage_wire::ensure_asset_id(id)?;
    }
    anyhow::ensure!(
        row.size_bytes > 0
            && row.expanded_bytes > 0
            && row.sha256.len() == 64
            && row.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid snapshot metadata"
    );
    let _: firemage_wire::SnapshotManifest = serde_json::from_str(&row.manifest)?;
    snapshots::Entity::insert(snapshots::ActiveModel {
        id: Set(row.id),
        owner_id: Set(row.owner_id),
        alias: Set(row.alias),
        source_vm_id: Set(row.source_vm_id),
        source_vm_name: Set(row.source_vm_name),
        created_at: Set(row.created_at),
        size_bytes: Set(row.size_bytes),
        expanded_bytes: Set(row.expanded_bytes),
        sha256: Set(row.sha256),
        trusted: Set(row.trusted),
        manifest: Set(row.manifest),
    })
    .exec(db)
    .await?;
    Ok(())
}
pub async fn trust_snapshot(db: &DatabaseConnection, owner: &str, id: &str) -> anyhow::Result<()> {
    let row = snapshot(db, owner, id).await?;
    let mut row: snapshots::ActiveModel = row.into();
    row.trusted = Set(true);
    row.update(db).await?;
    Ok(())
}
pub async fn delete_snapshot(db: &DatabaseConnection, owner: &str, id: &str) -> anyhow::Result<()> {
    let row = snapshot(db, owner, id).await?;
    snapshots::Entity::delete_by_id(row.id).exec(db).await?;
    Ok(())
}
