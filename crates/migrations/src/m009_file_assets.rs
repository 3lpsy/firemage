use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("file_assets"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("owner_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("alias")).string().not_null())
                    .col(ColumnDef::new(Alias::new("filename")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("size_bytes"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("sha256")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .name("file_assets_owner_alias")
                            .unique()
                            .col(Alias::new("owner_id"))
                            .col(Alias::new("alias")),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("file_assets"), Alias::new("owner_id"))
                            .to(Alias::new("users"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("file_assets")).to_owned())
            .await
    }
}
