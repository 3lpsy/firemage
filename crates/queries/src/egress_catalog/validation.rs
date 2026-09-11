use sea_orm::{ConnectionTrait, prelude::*};

#[derive(Debug)]
pub struct CatalogRevisionConflict;
impl std::fmt::Display for CatalogRevisionConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("catalog revision changed; reload before saving")
    }
}
impl std::error::Error for CatalogRevisionConflict {}
#[derive(Debug)]
pub struct CatalogInUse;
impl std::fmt::Display for CatalogInUse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("catalog resource is in use; detach its references before deleting")
    }
}
impl std::error::Error for CatalogInUse {}

pub(super) fn ids(owner: &str, id: &str) -> anyhow::Result<()> {
    firemage_wire::ensure_asset_id(owner)?;
    firemage_wire::ensure_asset_id(id)
}
pub(super) fn revision(value: u64) -> anyhow::Result<i64> {
    anyhow::ensure!(
        value > 0 && value < i64::MAX as u64,
        "invalid catalog revision"
    );
    Ok(value as i64)
}
pub(super) async fn secrets(
    db: &impl ConnectionTrait,
    owner: &str,
    names: Vec<&str>,
) -> anyhow::Result<()> {
    for name in names {
        firemage_wire::ensure_name(name)?;
        anyhow::ensure!(
            firemage_orm::secrets::Entity::find_by_id((owner.to_owned(), name.to_owned()))
                .one(db)
                .await?
                .is_some(),
            "secret {name} does not exist for this owner"
        );
    }
    Ok(())
}
