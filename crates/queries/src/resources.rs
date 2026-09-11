use firemage_orm::{networks, vms};
use sea_orm::{Set, TransactionTrait, prelude::*};

pub async fn insert_vm(
    db: &DatabaseConnection,
    owner: &str,
    name: &str,
    spec: String,
    socket: String,
) -> anyhow::Result<vms::Model> {
    let tx = db.begin().await?;
    let row = vms::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(owner.into()),
        name: Set(name.into()),
        spec: Set(spec),
        state: Set("defined".into()),
        error: Set(None),
        socket: Set(socket),
        pid: Set(None),
        process_start: Set(None),
    }
    .insert(&tx)
    .await?;
    crate::egress_catalog::bind_vm_egress(&tx, &row).await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn vm(db: &DatabaseConnection, owner: &str, id: &str) -> anyhow::Result<vms::Model> {
    vms::Entity::find_by_id(id)
        .filter(vms::Column::OwnerId.eq(owner))
        .one(db)
        .await?
        .ok_or_else(|| anyhow::Error::new(NotFound("VM")))
}
pub async fn vms(db: &DatabaseConnection, owner: Option<&str>) -> anyhow::Result<Vec<vms::Model>> {
    let mut query = vms::Entity::find();
    if let Some(owner) = owner {
        query = query.filter(vms::Column::OwnerId.eq(owner));
    }
    Ok(query.all(db).await?)
}
pub async fn set_vm_state(
    db: &DatabaseConnection,
    model: vms::Model,
    state: &str,
    error: Option<String>,
    pid: Option<i32>,
) -> anyhow::Result<vms::Model> {
    let mut active: vms::ActiveModel = model.into();
    active.state = Set(state.into());
    active.error = Set(error);
    active.pid = Set(pid);
    Ok(active.update(db).await?)
}
pub async fn delete_vm(db: &DatabaseConnection, owner: &str, id: &str) -> anyhow::Result<()> {
    vms::Entity::delete_many()
        .filter(vms::Column::Id.eq(id))
        .filter(vms::Column::OwnerId.eq(owner))
        .exec(db)
        .await?;
    Ok(())
}
pub async fn insert_network(
    db: &DatabaseConnection,
    owner: &str,
    name: &str,
    spec: String,
) -> anyhow::Result<networks::Model> {
    anyhow::ensure!(
        networks::Entity::find()
            .filter(networks::Column::Name.eq(name))
            .one(db)
            .await?
            .is_none(),
        "network name already exists"
    );
    Ok(networks::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(owner.into()),
        name: Set(name.into()),
        spec: Set(spec),
    }
    .insert(db)
    .await?)
}
pub async fn networks(
    db: &DatabaseConnection,
    owner: &str,
) -> anyhow::Result<Vec<networks::Model>> {
    Ok(networks::Entity::find()
        .filter(networks::Column::OwnerId.eq(owner))
        .all(db)
        .await?)
}
pub async fn network(
    db: &DatabaseConnection,
    owner: &str,
    name: &str,
) -> anyhow::Result<networks::Model> {
    networks::Entity::find()
        .filter(networks::Column::OwnerId.eq(owner))
        .filter(networks::Column::Name.eq(name))
        .one(db)
        .await?
        .ok_or_else(|| anyhow::Error::new(NotFound("network")))
}
pub async fn delete_network(
    db: &DatabaseConnection,
    owner: &str,
    name: &str,
) -> anyhow::Result<()> {
    networks::Entity::delete_many()
        .filter(networks::Column::OwnerId.eq(owner))
        .filter(networks::Column::Name.eq(name))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn set_process(
    db: &DatabaseConnection,
    id: &str,
    pid: i32,
    start: String,
) -> anyhow::Result<()> {
    vms::Entity::update_many()
        .col_expr(vms::Column::Pid, Expr::value(pid))
        .col_expr(vms::Column::ProcessStart, Expr::value(start))
        .filter(vms::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

#[derive(Debug)]
pub struct NotFound(pub &'static str);
impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} not found", self.0)
    }
}
impl std::error::Error for NotFound {}
