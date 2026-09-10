use crate::Runtime;
use anyhow::Context;
use firemage_firecracker::Firecracker;
use firemage_wire::{Asset, VmSpec};
use serde_json::json;

impl Runtime {
    pub(crate) async fn prepare(
        &self,
        row: &firemage_orm::vms::Model,
        fc: &Firecracker,
    ) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        spec.validate()?;
        self.validate_dependencies(&row.owner_id, &spec).await?;
        let seed_files = self.seed_files(&row.owner_id, &spec).await?;
        let dir = self.directory(&row.id);
        tokio::fs::create_dir_all(&dir).await?;
        anyhow::ensure!(
            !matches!(spec.kernel, Some(Asset::Oci { .. })),
            "kernel must be a local or remote binary"
        );
        let kernel = firemage_assets::materialize(
            spec.kernel.as_ref().context(
                "kernel required for prepare/start; use launch for manual configuration",
            )?,
            &dir.join("kernel"),
        )
        .await?;
        let rootfs = dir.join("rootfs.ext4");
        if !rootfs.exists() {
            firemage_assets::materialize(
                spec.rootfs
                    .as_ref()
                    .context("rootfs required for prepare/start")?,
                &rootfs,
            )
            .await?;
        }
        let initrd = if let Some(asset) = &spec.initrd {
            Some(firemage_assets::materialize(asset, &dir.join("initrd")).await?)
        } else {
            None
        };
        let mut boot_args = spec.boot_args.clone();
        if !seed_files.is_empty() || spec.userdata.is_some() {
            boot_args += " firemage.seed=1";
        }
        if matches!(spec.rootfs, Some(Asset::Oci { .. })) {
            boot_args += " init=/firemage-init";
        }
        let network = if let Some(net) = &spec.network {
            let definition =
                firemage_queries::network(&self.db, &row.owner_id, &net.network).await?;
            let definition: firemage_wire::NetworkSpec = serde_json::from_str(&definition.spec)?;
            let tap = self.network_tap(row, net).await?;
            boot_args += &format!(
                " ip={}::{}:{}::eth0:off",
                net.address,
                definition.gateway,
                definition.subnet.netmask()
            );
            Some((net, tap))
        } else {
            None
        };
        fc.call("PUT","/machine-config",json!({"vcpu_count":spec.vcpus,"mem_size_mib":spec.memory_mib,"track_dirty_pages":true})).await?;
        let mut boot = json!({"kernel_image_path":kernel,"boot_args":boot_args});
        if let Some(initrd) = initrd {
            boot["initrd_path"] = json!(initrd);
        }
        fc.call("PUT", "/boot-source", boot).await?;
        fc.call("PUT","/drives/rootfs",json!({"drive_id":"rootfs","path_on_host":rootfs,"is_root_device":true,"is_read_only":false})).await?;
        if let Some(seed) =
            firemage_assets::seed(&seed_files, spec.userdata.as_deref(), &dir).await?
        {
            fc.call("PUT","/drives/seed",json!({"drive_id":"seed","path_on_host":seed,"is_root_device":false,"is_read_only":true})).await?;
        }
        for drive in &spec.drives {
            let disk = dir.join(format!("drive-{}.img", drive.id));
            if !disk.exists() {
                firemage_assets::materialize(&drive.asset, &disk).await?;
            }
            fc.call("PUT",&format!("/drives/{}",drive.id),json!({"drive_id":drive.id,"path_on_host":disk,"is_root_device":false,"is_read_only":drive.read_only})).await?;
        }
        if let Some((net, tap)) = network {
            fc.call(
                "PUT",
                "/network-interfaces/eth0",
                json!({"iface_id":"eth0","host_dev_name":tap,"guest_mac":net.mac}),
            )
            .await?;
            if let Some(metadata) = &spec.metadata {
                fc.call(
                    "PUT",
                    "/mmds/config",
                    json!({"version":"V2","network_interfaces":["eth0"]}),
                )
                .await?;
                fc.call("PUT", "/mmds", metadata.clone()).await?;
            }
        }
        Ok(())
    }
    pub(crate) async fn launch(&self, row: &firemage_orm::vms::Model) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.socket.is_some() {
            return Ok(());
        }
        let args = self.config.firecracker_args.as_deref().unwrap_or_default();
        anyhow::ensure!(
            !args
                .iter()
                .any(|arg| matches!(arg.as_str(), "--api-sock" | "--no-api")
                    || arg.starts_with("--api-sock=")),
            "managed Firecracker requires its assigned API socket"
        );
        let socket = std::path::Path::new(&row.socket);
        tokio::fs::create_dir_all(socket.parent().context("socket parent")?).await?;
        if socket.exists() {
            anyhow::ensure!(
                Firecracker::new(socket)?.state().await.is_err(),
                "VM socket already has a live Firecracker process"
            );
            tokio::fs::remove_file(socket).await?;
        }
        tokio::fs::create_dir_all(self.directory(&row.id)).await?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory(&row.id).join("console.log"))?;
        let child = tokio::process::Command::new(
            self.config
                .firecracker
                .as_deref()
                .unwrap_or(std::path::Path::new("firecracker")),
        )
        .args(self.config.firecracker_args.as_deref().unwrap_or_default())
        .args(["--api-sock", &row.socket])
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
        let pid = child.id().map(|id| id as i32);
        self.children.lock().await.insert(row.id.clone(), child);
        let process_id = pid.context("missing Firecracker process id")?;
        firemage_queries::set_process(
            &self.db,
            &row.id,
            process_id,
            crate::process::identity(process_id)?,
        )
        .await?;
        firemage_queries::set_vm_state(&self.db, row.clone(), "starting", None, pid).await?;
        let fc = Firecracker::new(socket)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if fc.state().await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        anyhow::bail!("Firecracker socket did not become ready; inspect console log")
    }
}
