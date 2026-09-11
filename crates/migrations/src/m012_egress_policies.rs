use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("egress_policies"))
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
                    .col(ColumnDef::new(Alias::new("upstream_proxy_id")).string())
                    .index(
                        Index::create()
                            .name("egress_policies_owner_alias")
                            .unique()
                            .col(Alias::new("owner_id"))
                            .col(Alias::new("alias")),
                    )
                    .index(
                        Index::create()
                            .name("egress_policies_id_owner")
                            .unique()
                            .col(Alias::new("id"))
                            .col(Alias::new("owner_id")),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("egress_policies"), Alias::new("owner_id"))
                            .to(Alias::new("users"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from_tbl(Alias::new("egress_policies"))
                            .from_col(Alias::new("upstream_proxy_id"))
                            .from_col(Alias::new("owner_id"))
                            .to_tbl(Alias::new("upstream_proxies"))
                            .to_col(Alias::new("id"))
                            .to_col(Alias::new("owner_id"))
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
                    .table(Alias::new("egress_policies"))
                    .to_owned(),
            )
            .await
    }
}
