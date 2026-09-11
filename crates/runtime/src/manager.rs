use anyhow::Context;
use firemage_queries::DatabaseConnection;
use firemage_wire::{Vm, VmSpec, VmState};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    process::Child,
    sync::{Mutex, OnceCell},
};

#[derive(Clone)]
pub struct Runtime {
    pub db: DatabaseConnection,
    pub config: firemage_config::Server,
    pub(crate) children: Arc<Mutex<HashMap<String, Child>>>,
    pub(crate) secrets: Arc<OnceCell<firemage_secrets::Vault>>,
    pub(crate) egress: Arc<OnceCell<firemage_egress_proxy::EgressManager>>,
    locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}
impl Runtime {
    pub fn new(db: DatabaseConnection, config: firemage_config::Server) -> Self {
        Self {
            db,
            config,
            children: Default::default(),
            locks: Default::default(),
            secrets: Default::default(),
            egress: Default::default(),
        }
    }
    pub async fn lock(&self, id: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            self.locks
                .lock()
                .await
                .entry(id.into())
                .or_default()
                .clone()
        };
        lock.lock_owned().await
    }
    pub fn directory(&self, id: &str) -> PathBuf {
        self.config.data_dir().join("vms").join(id)
    }
    pub async fn define(&self, owner: &str, mut spec: VmSpec) -> anyhow::Result<Vm> {
        let _asset_guard = self.lock("file-assets").await;
        let _kernel_guard = self.lock("kernels").await;
        self.normalize_kernel(&mut spec)?;
        self.ensure_isolation_policy(&spec)?;
        let _network_guard = self.lock("networks").await;
        self.validate_dependencies(owner, &spec).await?;
        if let Some(net) = &spec.network {
            self.ensure_network_address_available(owner, net, None)
                .await?;
        }
        let socket = if let Some(socket) = &spec.socket {
            anyhow::ensure!(
                socket.is_absolute(),
                "Firecracker socket path must be absolute"
            );
            tokio::fs::canonicalize(socket)
                .await
                .context("attached socket must exist")?
        } else {
            self.config
                .data_dir()
                .join("sockets")
                .join(format!("{}.sock", uuid::Uuid::new_v4()))
        };
        anyhow::ensure!(
            socket.as_os_str().len() < 108,
            "Unix socket path exceeds Linux limit"
        );
        let row = firemage_queries::insert_vm(
            &self.db,
            owner,
            &spec.name,
            serde_json::to_string(&spec)?,
            socket.to_str().context("invalid socket path")?.into(),
        )
        .await?;
        view(row)
    }
    pub async fn refresh(
        &self,
        mut row: firemage_orm::vms::Model,
    ) -> anyhow::Result<firemage_orm::vms::Model> {
        if row.state == "failed"
            || (row.state == "defined" && !std::path::Path::new(&row.socket).exists())
        {
            return Ok(row);
        }
        let state = match firemage_firecracker::Firecracker::new(std::path::Path::new(&row.socket))?
            .state()
            .await
        {
            Ok(state) => {
                let spec: VmSpec = serde_json::from_str(&row.spec)?;
                if spec.security.mode == firemage_wire::IsolationMode::Jailed
                    && let Err(error) = self.ensure_jail_process(&row, state != "Not started").await
                {
                    let _ = firemage_network::suspend(&row.id).await;
                    let pid = row.pid;
                    return firemage_queries::set_vm_state(&self.db, row, "unknown", Some(format!("VM isolation verification failed: {error}; stop and restart after correcting host configuration")), pid).await;
                }
                if row.pid.is_none()
                    && spec.socket.is_none()
                    && let Ok((pid, start)) = crate::process::from_socket(&row.socket).await
                {
                    firemage_queries::set_process(&self.db, &row.id, pid, start.clone()).await?;
                    row.pid = Some(pid);
                    row.process_start = Some(start);
                }
                match state.as_str() {
                    "Running" => "running",
                    "Paused" => "paused",
                    "Not started" => "ready",
                    _ => "unknown",
                }
            }
            Err(_) => {
                let mut children = self.children.lock().await;
                if let Some(child) = children.get_mut(&row.id) {
                    if child.try_wait()?.is_some() {
                        children.remove(&row.id);
                        "stopped"
                    } else {
                        "unknown"
                    }
                } else if row
                    .pid
                    .zip(row.process_start.as_deref())
                    .is_some_and(|(pid, start)| !crate::process::is_alive(pid, start))
                    || row.state == "stopped"
                {
                    "stopped"
                } else {
                    "unknown"
                }
            }
        };
        if state == "stopped" && row.state != "stopped" {
            self.unregister_egress(&row.id).await;
            let spec: VmSpec = serde_json::from_str(&row.spec)?;
            if spec.network.is_some() {
                let _ = firemage_network::remove(&row.id).await;
            }
        }
        let pid = if state == "stopped" { None } else { row.pid };
        let error = if row.state == state {
            row.error.clone()
        } else {
            None
        };
        firemage_queries::set_vm_state(&self.db, row, state, error, pid).await
    }
    pub async fn delete(&self, owner: &str, id: &str) -> anyhow::Result<()> {
        self.delete_with_snapshots(owner, id, false).await
    }
    pub async fn delete_with_snapshots(
        &self,
        owner: &str,
        id: &str,
        snapshots: bool,
    ) -> anyhow::Result<()> {
        let _guard = self.lock(id).await;
        let row = self
            .refresh(firemage_queries::vm(&self.db, owner, id).await?)
            .await?;
        anyhow::ensure!(
            matches!(row.state.as_str(), "defined" | "stopped" | "failed")
                && !self.children.lock().await.contains_key(id),
            "stop VM before deleting it"
        );
        self.ensure_stopped_process(&row).await?;
        self.unregister_egress(id).await;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.network.is_some() {
            let _ = firemage_network::remove(id).await;
        }
        self.cleanup_cgroup(&row).await?;
        if self.jail_root(&row).exists() {
            tokio::fs::remove_dir_all(self.jail_root(&row)).await?;
        }
        let _ =
            tokio::fs::remove_file(self.config.data_dir().join("jailer-identities").join(id)).await;
        if self.directory(id).exists() {
            tokio::fs::remove_dir_all(self.directory(id)).await?;
        }
        if spec.socket.is_none() {
            let _ = tokio::fs::remove_file(&row.socket).await;
        }
        if snapshots {
            self.delete_vm_snapshots(owner, id).await?;
        }
        firemage_queries::delete_vm(&self.db, owner, id).await
    }
}
pub fn view(row: firemage_orm::vms::Model) -> anyhow::Result<Vm> {
    Ok(Vm {
        id: row.id,
        owner_id: row.owner_id,
        spec: serde_json::from_str(&row.spec)?,
        state: serde_json::from_value(serde_json::Value::String(row.state))
            .unwrap_or(VmState::Unknown),
        error: row.error,
    })
}
