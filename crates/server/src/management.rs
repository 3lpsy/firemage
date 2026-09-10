use crate::{App, error::Result, identity::Identity};
use axum::{Json, extract::State};
use serde_json::{Value, json};

impl App {
    pub(crate) async fn record(
        &self,
        identity: &Identity,
        action: &str,
        resource: &str,
    ) -> anyhow::Result<()> {
        firemage_queries::record_activity(
            &self.runtime.db,
            &identity.user.id,
            &identity.user.username,
            action,
            resource,
        )
        .await
    }
}
pub async fn configuration(
    identity: Identity,
    State(app): State<App>,
) -> Result<Json<firemage_config::ConfigView>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    Ok(Json(app.management.view()?))
}
pub async fn validate(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<firemage_config::ConfigEdit>,
) -> Result<Json<firemage_config::ConfigView>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    Ok(Json(app.management.edit(input, false)?))
}
pub async fn save(
    identity: Identity,
    State(app): State<App>,
    Json(input): Json<firemage_config::ConfigEdit>,
) -> Result<Json<firemage_config::ConfigView>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    let result = app.management.edit(input, true)?;
    app.record(&identity, "config.save", "server").await?;
    Ok(Json(result))
}
pub async fn activity(identity: Identity, State(app): State<App>) -> Result<Json<Vec<Value>>> {
    let actor = if identity.user.admin {
        None
    } else {
        Some(identity.user.id.as_str())
    };
    Ok(Json(firemage_queries::activity(&app.runtime.db, actor).await?.into_iter().map(|row| json!({"id":row.id,"at":row.at,"actor":row.actor,"action":row.action,"resource":row.resource})).collect()))
}
pub async fn host(identity: Identity, State(app): State<App>) -> Result<Json<Value>> {
    let owner = if identity.user.admin {
        None
    } else {
        Some(identity.user.id.as_str())
    };
    let vms = firemage_queries::vms(&app.runtime.db, owner).await?;
    let mut counts = json!({"total":vms.len(),"running":0,"paused":0,"stopped":0,"failed":0,"defined":0,"ready":0,"unknown":0,"starting":0});
    for vm in vms {
        if let Some(count) = counts.get_mut(vm.state) {
            *count = json!(count.as_u64().unwrap_or(0) + 1);
        }
    }
    let memory = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let read_memory = |key: &str| {
        memory
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            * 1024
    };
    let networks = if identity.user.admin {
        firemage_queries::all_networks(&app.runtime.db).await?.len()
    } else {
        firemage_queries::networks(&app.runtime.db, &identity.user.id)
            .await?
            .len()
    };
    let users = if identity.user.admin {
        Some(firemage_queries::users(&app.runtime.db).await?.len())
    } else {
        None
    };
    Ok(Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"uptime_seconds":app.started.elapsed().as_secs(),"kvm_available":std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm").is_ok(),"cpus":std::thread::available_parallelism().map(usize::from).unwrap_or(1),"memory_total_bytes":read_memory("MemTotal:"),"memory_available_bytes":read_memory("MemAvailable:"),"vms":counts,"networks":networks,"users":users}),
    ))
}
