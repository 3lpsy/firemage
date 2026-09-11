use crate::Runtime;
use firemage_wire::VmSpec;

impl Runtime {
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
                    if self.is_process_stopped(&row).await? {
                        return self.record_stopped(row).await;
                    }
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
                if self.is_process_stopped(&row).await? || row.state == "stopped" {
                    "stopped"
                } else {
                    "unknown"
                }
            }
        };
        if state == "stopped" {
            return self.record_stopped(row).await;
        }
        let pid = row.pid;
        let error = if row.state == state {
            row.error.clone()
        } else {
            None
        };
        firemage_queries::set_vm_state(&self.db, row, state, error, pid).await
    }

    async fn is_process_stopped(&self, row: &firemage_orm::vms::Model) -> anyhow::Result<bool> {
        let mut children = self.children.lock().await;
        if let Some(child) = children.get_mut(&row.id) {
            if child.try_wait()?.is_some() {
                children.remove(&row.id);
                return Ok(true);
            }
            return Ok(false);
        }
        Ok(row
            .pid
            .zip(row.process_start.as_deref())
            .is_some_and(|(pid, start)| !crate::process::is_alive(pid, start)))
    }

    async fn record_stopped(
        &self,
        row: firemage_orm::vms::Model,
    ) -> anyhow::Result<firemage_orm::vms::Model> {
        if row.state != "stopped" {
            self.unregister_egress(&row.id).await;
            let spec: VmSpec = serde_json::from_str(&row.spec)?;
            if spec.network.is_some() {
                let _ = firemage_network::remove(&row.id).await;
            }
        }
        let error = if row.state == "stopped" {
            row.error.clone()
        } else {
            None
        };
        firemage_queries::set_vm_state(&self.db, row, "stopped", error, None).await
    }
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
