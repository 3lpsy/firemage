use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("file_assets"))
                    .add_column(ColumnDef::new(Alias::new("storage_name")).string().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("file_assets_storage_name")
                    .table(Alias::new("file_assets"))
                    .col(Alias::new("storage_name"))
                    .unique()
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("file_assets_storage_name")
                    .table(Alias::new("file_assets"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("file_assets"))
                    .drop_column(Alias::new("storage_name"))
                    .to_owned(),
            )
            .await
    }
}
