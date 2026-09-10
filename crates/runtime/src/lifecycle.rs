use crate::{Runtime, view};
use anyhow::Context;
use firemage_firecracker::Firecracker;
use firemage_wire::{Vm, VmAction, VmSpec};
use serde_json::json;

impl Runtime {
    pub async fn action(&self, owner: &str, id: &str, action: VmAction) -> anyhow::Result<Vm> {
        let _guard = self.lock(id).await;
        let mut row = firemage_queries::vm(&self.db, owner, id).await?;
        if matches!(action, VmAction::Refresh) {
            return view(self.refresh(row).await?);
        }
        let fc = Firecracker::new(std::path::Path::new(&row.socket))?;
        let result = async {
            match action {
                VmAction::Launch | VmAction::Prepare | VmAction::Start => {
                    let is_start = matches!(action,VmAction::Start);
                    let is_launch = matches!(action,VmAction::Launch);
                    anyhow::ensure!(matches!(row.state.as_str(),"defined"|"stopped"|"failed") || (is_start && row.state == "ready"), "VM cannot launch from {}",row.state);
                    let spec: VmSpec = serde_json::from_str(&row.spec)?;
                    anyhow::ensure!(!is_launch || !self.is_restricted_network(owner, &spec).await?, "Firemage-only VMs require prepare or start");
                    if row.state != "ready" {
                        row = firemage_queries::set_vm_state(&self.db,row.clone(),"starting",None,None).await?;
                        self.launch(&row).await?;
                        if !is_launch { self.prepare(&row,&fc).await?; }
                    }
                    if is_start { fc.call("PUT","/actions",json!({"action_type":"InstanceStart"})).await?; Ok("running") } else { Ok("ready") }
                },
                VmAction::Pause => { anyhow::ensure!(fc.state().await? == "Running", "VM must be running"); fc.call("PATCH","/vm",json!({"state":"Paused"})).await?; Ok("paused") },
                VmAction::Resume => { anyhow::ensure!(fc.state().await? == "Paused", "VM must be paused"); fc.call("PATCH","/vm",json!({"state":"Resumed"})).await?; Ok("running") },
                VmAction::Shutdown => { fc.call("PUT","/actions",json!({"action_type":"SendCtrlAltDel"})).await?; Ok("running") },
                VmAction::Stop => {
                    if row.state != "stopped" { self.stop_process(&row).await?; }
                    self.unregister_egress(id).await;
                    let spec: VmSpec = serde_json::from_str(&row.spec)?;
                    if spec.network.is_some() { let _ = firemage_network::remove(id).await; }
                    Ok("stopped")
                },
                VmAction::Snapshot { snapshot_path,memory_path } => {
                    anyhow::ensure!(fc.state().await? == "Paused", "pause VM before snapshotting");
                    ensure_absolute(&snapshot_path)?; ensure_absolute(&memory_path)?;
                    fc.call("PUT","/snapshot/create",json!({"snapshot_type":"Full","snapshot_path":snapshot_path,"mem_file_path":memory_path})).await?;
                    self.record_snapshot(&row, &snapshot_path, &memory_path).await?;
                    Ok("paused")
                },
                VmAction::Restore { snapshot_path,memory_path } => {
                    anyhow::ensure!(matches!(row.state.as_str(),"defined"|"stopped"), "restore requires a fresh VMM");
                    self.ensure_snapshot(&row, &snapshot_path, &memory_path).await?;
                    ensure_absolute(&snapshot_path)?; ensure_absolute(&memory_path)?;
                    row = firemage_queries::set_vm_state(&self.db,row.clone(),"starting",None,None).await?;
                    self.launch(&row).await?;
                    let spec: VmSpec = serde_json::from_str(&row.spec)?;
                    if let Some(net) = &spec.network {
                        self.network_tap(&row, net).await?;
                    }
                    fc.call("PUT","/snapshot/load",json!({"snapshot_path":snapshot_path,"mem_backend":{"backend_type":"File","backend_path":memory_path},"resume_vm":false})).await?; Ok("paused")
                },
                VmAction::Metadata { value } => { fc.call("PUT","/mmds",value).await?; Ok(row.state.as_str()) },
                VmAction::Refresh => unreachable!(),
            }
        }.await;
        match result {
            Ok(state) => {
                let state = state.to_owned();
                let pid = self
                    .children
                    .lock()
                    .await
                    .get(id)
                    .and_then(|c| c.id())
                    .map(|p| p as i32)
                    .or(row.pid)
                    .filter(|_| state != "stopped");
                view(firemage_queries::set_vm_state(&self.db, row, &state, None, pid).await?)
            }
            Err(error) => {
                if row.state == "starting" {
                    let stopped = self.stop_process(&row).await.is_ok();
                    if stopped {
                        self.unregister_egress(id).await;
                        let _ = firemage_network::remove(id).await;
                    }
                    firemage_queries::set_vm_state(
                        &self.db,
                        row,
                        if stopped { "failed" } else { "unknown" },
                        Some(error.to_string()),
                        None,
                    )
                    .await?;
                }
                Err(error)
            }
        }
    }
    async fn stop_process(&self, row: &firemage_orm::vms::Model) -> anyhow::Result<()> {
        let mut children = self.children.lock().await;
        if let Some(child) = children.get_mut(&row.id) {
            child.kill().await?;
            child.wait().await?;
            children.remove(&row.id);
        } else {
            let spec: VmSpec = serde_json::from_str(&row.spec)?;
            if row.pid.is_none()
                && spec.socket.is_none()
                && !std::path::Path::new(&row.socket).exists()
            {
                return Ok(());
            }
            let pid = row
                .pid
                .context("attached VM process cannot be force-stopped; use shutdown")?;
            let start = row
                .process_start
                .clone()
                .context("missing process identity")?;
            if crate::process::is_alive(pid, &start) {
                crate::process::stop(pid, start).await?;
            }
        }
        Ok(())
    }
    pub async fn output(&self, owner: &str, id: &str, path: &str) -> anyhow::Result<Vec<u8>> {
        let _guard = self.lock(id).await;
        let row = self
            .refresh(firemage_queries::vm(&self.db, owner, id).await?)
            .await?;
        anyhow::ensure!(
            row.state == "stopped",
            "file extraction requires a stopped VM"
        );
        let destination = self
            .directory(id)
            .join(format!("extract-{}", uuid::Uuid::new_v4()));
        let result = async {
            firemage_assets::extract(&self.directory(id).join("rootfs.ext4"), path, &destination)
                .await?;
            anyhow::ensure!(
                tokio::fs::metadata(&destination).await?.len() <= 64 * 1024 * 1024,
                "output file exceeds 64 MiB"
            );
            Ok(tokio::fs::read(&destination).await?)
        }
        .await;
        let _ = tokio::fs::remove_file(destination).await;
        result
    }
}
fn ensure_absolute(path: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        std::path::Path::new(path).is_absolute() && !path.contains('\0'),
        "snapshot paths must be absolute host paths"
    );
    Ok(())
}
