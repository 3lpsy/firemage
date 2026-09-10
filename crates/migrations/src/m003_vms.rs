use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("vms"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("owner_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("name")).string().not_null())
                    .col(ColumnDef::new(Alias::new("spec")).string().not_null())
                    .col(ColumnDef::new(Alias::new("state")).string().not_null())
                    .col(ColumnDef::new(Alias::new("error")).string())
                    .col(
                        ColumnDef::new(Alias::new("socket"))
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Alias::new("pid")).integer())
                    .col(ColumnDef::new(Alias::new("process_start")).string())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("vms"), Alias::new("owner_id"))
                            .to(Alias::new("users"), Alias::new("id")),
                    )
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("vms")).to_owned())
            .await
    }
}
