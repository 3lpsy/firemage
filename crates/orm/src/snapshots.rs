use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "snapshots")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub owner_id: String,
    pub alias: String,
    pub source_vm_id: Option<String>,
    pub source_vm_name: String,
    pub created_at: i64,
    pub size_bytes: i64,
    pub expanded_bytes: i64,
    pub sha256: String,
    pub trusted: bool,
    pub manifest: String,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
