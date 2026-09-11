use crate::Runtime;
use firemage_firecracker::Firecracker;
use firemage_wire::{Asset, VmSpec};
use serde_json::json;

impl Runtime {
    pub(crate) async fn is_preparation_required(&self, fc: &Firecracker) -> anyhow::Result<bool> {
        let config = fc
            .call("GET", "/vm/config", serde_json::Value::Null)
            .await?;
        let kernel = config["boot-source"]["kernel_image_path"]
            .as_str()
            .is_some_and(|path| !path.is_empty());
        let root = config["drives"]
            .as_array()
            .is_some_and(|drives| drives.iter().any(|drive| drive["is_root_device"] == true));
        Ok(!kernel || !root)
    }
    pub(crate) async fn prepare(
        &self,
        row: &firemage_orm::vms::Model,
        fc: &Firecracker,
    ) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        spec.validate()?;
        self.validate_dependencies(&row.owner_id, &spec).await?;
        if spec.security.mode != firemage_wire::IsolationMode::Jailed {
            self.materialize_assets(row, &spec).await?;
        }
        let dir = self.directory(&row.id);
        let kernel = self.resource_path(row, "kernel", false).await?;
        let rootfs = self.resource_path(row, "rootfs.ext4", true).await?;
        let initrd = if spec.initrd.is_some() {
            Some(self.resource_path(row, "initrd", false).await?)
        } else {
            None
        };
        let mut boot_args = spec.boot_args.clone();
        if dir.join("seed.ext4").exists() {
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
        if dir.join("seed.ext4").exists() {
            let seed = self.resource_path(row, "seed.ext4", false).await?;
            fc.call("PUT","/drives/seed",json!({"drive_id":"seed","path_on_host":seed,"is_root_device":false,"is_read_only":true})).await?;
        }
        for drive in &spec.drives {
            let disk = self
                .resource_path(row, &format!("drive-{}.img", drive.id), !drive.read_only)
                .await?;
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
}
