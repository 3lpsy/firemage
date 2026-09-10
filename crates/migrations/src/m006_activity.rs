use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("activity"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("at")).big_integer().not_null())
                    .col(ColumnDef::new(Alias::new("actor_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("actor")).string().not_null())
                    .col(ColumnDef::new(Alias::new("action")).string().not_null())
                    .col(ColumnDef::new(Alias::new("resource")).string().not_null())
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("activity")).to_owned())
            .await
    }
}
