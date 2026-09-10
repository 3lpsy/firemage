use sea_orm::entity::prelude::*;
#[derive(Clone, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "secrets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub owner_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub name: String,
    pub ciphertext: String,
    pub updated_at: i64,
}
impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret")
            .field("owner_id", &self.owner_id)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
