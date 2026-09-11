use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("upstream_proxies"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("owner_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("alias")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("revision"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("spec")).text().not_null())
                    .col(ColumnDef::new(Alias::new("ca_secret")).string())
                    .index(
                        Index::create()
                            .name("upstream_proxies_owner_alias")
                            .unique()
                            .col(Alias::new("owner_id"))
                            .col(Alias::new("alias")),
                    )
                    .index(
                        Index::create()
                            .name("upstream_proxies_id_owner")
                            .unique()
                            .col(Alias::new("id"))
                            .col(Alias::new("owner_id")),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("upstream_proxies"), Alias::new("owner_id"))
                            .to(Alias::new("users"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("upstream_proxies"))
                    .to_owned(),
            )
            .await
    }
}
