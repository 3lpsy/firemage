use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("snapshots"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("owner_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("alias")).string().not_null())
                    .col(ColumnDef::new(Alias::new("source_vm_id")).string())
                    .col(
                        ColumnDef::new(Alias::new("source_vm_name"))
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("size_bytes"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("expanded_bytes"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("sha256")).string().not_null())
                    .col(ColumnDef::new(Alias::new("trusted")).boolean().not_null())
                    .col(ColumnDef::new(Alias::new("manifest")).text().not_null())
                    .index(
                        Index::create()
                            .name("snapshots_owner_alias")
                            .unique()
                            .col(Alias::new("owner_id"))
                            .col(Alias::new("alias")),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("snapshots"), Alias::new("owner_id"))
                            .to(Alias::new("users"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("snapshots")).to_owned())
            .await
    }
}
