use firemage_orm::kernel_aliases;
use sea_orm::{Set, prelude::*, sea_query::OnConflict};

pub async fn kernel_aliases(db: &DatabaseConnection) -> anyhow::Result<Vec<kernel_aliases::Model>> {
    Ok(kernel_aliases::Entity::find().all(db).await?)
}
pub async fn set_kernel_alias(
    db: &DatabaseConnection,
    name: &str,
    alias: Option<&str>,
) -> anyhow::Result<()> {
    firemage_wire::ensure_kernel_name(name)?;
    if let Some(alias) = alias {
        firemage_wire::KernelAlias {
            alias: Some(alias.to_owned()),
        }
        .validate()?;
        kernel_aliases::Entity::insert(kernel_aliases::ActiveModel {
            name: Set(name.into()),
            alias: Set(alias.into()),
        })
        .on_conflict(
            OnConflict::column(kernel_aliases::Column::Name)
                .update_column(kernel_aliases::Column::Alias)
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    } else {
        kernel_aliases::Entity::delete_by_id(name).exec(db).await?;
    }
    Ok(())
}
