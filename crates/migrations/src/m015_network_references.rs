use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, TransactionTrait},
};
use serde_json::Value;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rewrite(manager, false).await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rewrite(manager, true).await
    }
}

async fn rewrite(manager: &SchemaManager<'_>, reverse: bool) -> Result<(), DbErr> {
    let tx = manager.get_connection().begin().await?;
    let backend = tx.get_database_backend();
    let networks = tx
        .query_all(
            backend.build(
                &Query::select()
                    .columns(["id", "owner_id", "name"].map(Alias::new))
                    .from(Alias::new("networks"))
                    .to_owned(),
            ),
        )
        .await?;
    let networks = networks
        .iter()
        .map(|row| {
            Ok((
                row.try_get::<String>("", "id")?,
                row.try_get::<String>("", "owner_id")?,
                row.try_get::<String>("", "name")?,
            ))
        })
        .collect::<Result<Vec<_>, DbErr>>()?;
    let vms = tx
        .query_all(
            backend.build(
                &Query::select()
                    .columns(["id", "owner_id", "spec"].map(Alias::new))
                    .from(Alias::new("vms"))
                    .to_owned(),
            ),
        )
        .await?;
    for row in vms {
        let id: String = row.try_get("", "id")?;
        let owner: String = row.try_get("", "owner_id")?;
        let source: String = row.try_get("", "spec")?;
        let mut spec: Value = serde_json::from_str(&source).map_err(|_| {
            DbErr::Custom(format!(
                "invalid VM configuration during network migration: {id}"
            ))
        })?;
        if matches!(spec.get("network"), None | Some(Value::Null)) {
            continue;
        }
        let reference = spec["network"]["network"]
            .as_str()
            .ok_or_else(|| DbErr::Custom(format!("invalid network reference in VM {id}")))?;
        let matches = |network: &&(String, String, String)| network.1 == owner;
        let by_id = networks
            .iter()
            .filter(matches)
            .find(|network| network.0 == reference);
        let by_name = networks
            .iter()
            .filter(matches)
            .find(|network| network.2 == reference);
        if by_id.is_some() && by_name.is_some() && by_id != by_name {
            return Err(DbErr::Custom(format!(
                "ambiguous network reference in VM {id}"
            )));
        }
        let network = by_id.or(by_name).ok_or_else(|| {
            DbErr::Custom(format!("VM {id} references a missing or foreign network"))
        })?;
        spec["network"]["network"] =
            Value::String(if reverse { &network.2 } else { &network.0 }.clone());
        tx.execute(
            backend.build(
                &Query::update()
                    .table(Alias::new("vms"))
                    .value(Alias::new("spec"), spec.to_string())
                    .and_where(Expr::col(Alias::new("id")).eq(id))
                    .to_owned(),
            ),
        )
        .await?;
    }
    let schema = SchemaManager::new(&tx);
    if reverse {
        schema
            .drop_index(
                Index::drop()
                    .name("networks_unique_name")
                    .table(Alias::new("networks"))
                    .to_owned(),
            )
            .await?;
    } else {
        schema
            .create_index(
                Index::create()
                    .name("networks_unique_name")
                    .table(Alias::new("networks"))
                    .col(Alias::new("name"))
                    .unique()
                    .to_owned(),
            )
            .await?;
    }
    tx.commit().await
}
