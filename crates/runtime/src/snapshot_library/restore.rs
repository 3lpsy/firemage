use super::{compatibility, restore_files::DiskSwap};
use crate::{Runtime, view};
use anyhow::Context;
use firemage_firecracker::Firecracker;
use firemage_wire::{SnapshotManifest, Vm, VmSpec};
use serde_json::{Value, json};

impl Runtime {
    pub async fn restore_snapshot(
        &self,
        owner: &str,
        id: &str,
        snapshot_id: &str,
    ) -> anyhow::Result<Vm> {
        self.restore_snapshot_from(owner, id, owner, snapshot_id)
            .await
    }

    pub async fn restore_snapshot_from(
        &self,
        owner: &str,
        id: &str,
        snapshot_owner: &str,
        snapshot_id: &str,
    ) -> anyhow::Result<Vm> {
        let _guard = self.lock(id).await;
        let row = self
            .refresh(firemage_queries::vm(&self.db, owner, id).await?)
            .await?;
        anyhow::ensure!(
            matches!(row.state.as_str(), "defined" | "stopped" | "failed"),
            "snapshot restore requires a stopped VM"
        );
        self.ensure_stopped_process(&row).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        self.ensure_isolation_policy(&spec)?;
        let directory = self.directory(id);
        tokio::fs::create_dir_all(&directory).await?;
        let stage = tempfile::Builder::new()
            .prefix("restore-stage-")
            .tempdir_in(&directory)?;
        let manifest = {
            let _catalog_guard = self.lock("snapshot-library").await;
            self.verified_snapshot(snapshot_owner, snapshot_id, stage.path())
                .await?
        };
        compatibility::ensure_target(&manifest, &spec)?;
        self.ensure_snapshot_network(owner, &manifest, &spec)
            .await?;
        let mut swap = DiskSwap::install(&directory, stage.path(), &manifest)?;
        let attempt = async {
            let starting = firemage_queries::set_vm_state(&self.db, row.clone(), "starting", None, None).await?;
            super::restore_files::preserve_diagnostics(&directory)?;
            self.launch_snapshot_with_assets(&starting, &manifest.spec).await?;
            let fc = Firecracker::new(std::path::Path::new(&row.socket))?;
            let version = fc.call("GET", "/version", Value::Null).await?;
            anyhow::ensure!(version["firecracker_version"] == manifest.firecracker_version, "snapshot Firecracker version differs from this host");
            let mut overrides = Vec::new();
            if let Some(net) = &spec.network {
                let tap = self.network_tap(&starting, net).await?;
                firemage_network::set_tap_mac(id, manifest.gateway_mac.as_deref().context("missing snapshot gateway MAC")?).await?;
                overrides.push(json!({"iface_id":"eth0","host_dev_name":tap}));
            }
            let (state, memory) = self.snapshot_import(&starting,
                stage.path().join("state.bin").to_str().context("invalid snapshot path")?,
                stage.path().join("memory.bin").to_str().context("invalid memory path")?).await?;
            let mut load = json!({"snapshot_path":state,"mem_backend":{"backend_type":"File","backend_path":memory},"resume_vm":false,"network_overrides":overrides});
            if spec.web_terminal.is_some() { load["vsock_override"] = json!({"uds_path": self.shell_device_path(&starting, &spec)}); }
            fc.call("PUT", "/snapshot/load", load).await?;
            let actual = compatibility::ensure_devices(&fc, &spec).await?;
            let disks = actual["drives"].as_array().context("missing restored disks")?;
            anyhow::ensure!(disks.iter().any(|drive| drive["drive_id"] == "seed") == manifest.files.contains_key("seed.ext4"), "restored seed device differs from snapshot bundle");
            if spec.network.is_some() {
                anyhow::ensure!(actual["network-interfaces"][0]["host_dev_name"] == firemage_network::identifiers(id)?.0, "restored network did not use assigned TAP");
            }
            if let Some(metadata) = &spec.metadata {
                fc.call("PUT", "/mmds", metadata.clone()).await?;
            }
            anyhow::ensure!(fc.state().await? == "Paused", "snapshot did not restore paused");
            self.ensure_jail_process(&starting, true).await?;
            let current = firemage_queries::vm(&self.db, owner, id).await?;
            let pid = current.pid;
            view(firemage_queries::set_vm_state(&self.db, current, "paused", None, pid).await?)
        }.await;
        match attempt {
            Ok(vm) => {
                // A backup cleanup failure must not report a successfully restored VM as failed.
                if let Err(error) = swap.commit() {
                    eprintln!("snapshot restore backup cleanup failed: {error}");
                }
                Ok(vm)
            }
            Err(error) => {
                let current = firemage_queries::vm(&self.db, owner, id).await?;
                let stopped = self.stop_process(&current).await.is_ok();
                if stopped {
                    self.unregister_egress(id).await;
                    if spec.network.is_some() {
                        let _ = firemage_network::remove(id).await;
                    }
                    let _ = self.cleanup_cgroup(&current).await;
                    if let Err(rollback) = swap.rollback() {
                        firemage_queries::set_vm_state(
                            &self.db,
                            current,
                            "unknown",
                            Some(format!("{error}; resource rollback failed: {rollback}")),
                            None,
                        )
                        .await?;
                        anyhow::bail!("{error}; resource rollback failed: {rollback}");
                    }
                }
                let retained_pid = if stopped { None } else { current.pid };
                firemage_queries::set_vm_state(
                    &self.db,
                    current,
                    if stopped { "failed" } else { "unknown" },
                    Some(error.to_string()),
                    retained_pid,
                )
                .await?;
                if !stopped {
                    anyhow::bail!(
                        "{error}; process cleanup failed, original resources retained at {}",
                        swap.backup.display()
                    );
                }
                Err(error)
            }
        }
    }

    pub(super) async fn ensure_snapshot_network(
        &self,
        owner: &str,
        manifest: &SnapshotManifest,
        target: &VmSpec,
    ) -> anyhow::Result<()> {
        if let Some(network) = &manifest.network {
            let attachment = target.network.as_ref().context("missing target network")?;
            let current = firemage_queries::network(&self.db, owner, &attachment.network).await?;
            let current: firemage_wire::NetworkSpec = serde_json::from_str(&current.spec)?;
            anyhow::ensure!(
                current.subnet.trunc() == network.subnet.trunc()
                    && current.gateway == network.gateway
                    && serde_json::to_value(&current.policy)?
                        == serde_json::to_value(&network.policy)?,
                "target network subnet, gateway or policy differs from the snapshot"
            );
        }
        Ok(())
    }
}
