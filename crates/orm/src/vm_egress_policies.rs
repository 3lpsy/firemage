use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "vm_egress_policies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub vm_id: String,
    pub owner_id: String,
    pub policy_id: String,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
