use crate::Runtime;
use anyhow::Context;
use firemage_wire::{IsolationMode, Snapshot, SnapshotFile, SnapshotManifest, VmSpec};
use std::collections::BTreeMap;

impl Runtime {
    pub async fn save_snapshot(
        &self,
        owner: &str,
        id: &str,
        alias: &str,
    ) -> anyhow::Result<Snapshot> {
        firemage_wire::ensure_asset_alias(alias)?;
        anyhow::ensure!(
            !self
                .snapshots(owner)
                .await?
                .iter()
                .any(|snapshot| snapshot.alias == alias),
            "snapshot alias already exists"
        );
        let _guard = self.lock(id).await;
        let row = self
            .refresh(firemage_queries::vm(&self.db, owner, id).await?)
            .await?;
        anyhow::ensure!(
            row.state == "paused",
            "pause the VM before saving a snapshot"
        );
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        anyhow::ensure!(
            spec.security.mode == IsolationMode::Jailed,
            "portable snapshots require a managed jailed VM"
        );
        self.ensure_jail_process(&row, true).await?;
        let fc = firemage_firecracker::Firecracker::new(std::path::Path::new(&row.socket))?;
        let version = fc.call("GET", "/version", serde_json::Value::Null).await?;
        let version = version["firecracker_version"]
            .as_str()
            .context("missing Firecracker version")?
            .to_owned();
        let devices = super::compatibility::ensure_devices(&fc, &spec).await?;
        if spec.network.is_some() {
            anyhow::ensure!(
                devices["network-interfaces"][0]["host_dev_name"].as_str()
                    == Some(&firemage_network::identifiers(id)?.0),
                "VM TAP does not match its managed network"
            );
        }
        let mut names = vec!["kernel".to_owned(), "rootfs.ext4".into()];
        for optional in ["initrd", "oci-init-version", "egress-bootstrap"] {
            if self.directory(id).join(optional).try_exists()? {
                names.push(optional.into());
            }
        }
        if devices["drives"]
            .as_array()
            .is_some_and(|drives| drives.iter().any(|drive| drive["drive_id"] == "seed"))
        {
            names.push("seed.ext4".into());
        }
        names.extend(
            spec.drives
                .iter()
                .map(|drive| format!("drive-{}.img", drive.id)),
        );
        let disk_bytes = names.iter().try_fold(0u64, |total, name| {
            let size = firemage_snapshots::open_private(&self.directory(id).join(name))?
                .metadata()?
                .len();
            total.checked_add(size).context("snapshot size overflow")
        })?;
        anyhow::ensure!(
            disk_bytes
                .checked_add(u64::from(spec.memory_mib) * 1024 * 1024)
                .is_some_and(|size| size <= self.config.snapshot_max_bytes()),
            "snapshot exceeds configured size limit"
        );
        let stage = tempfile::Builder::new()
            .prefix("capture-")
            .tempdir_in(self.snapshot_directory()?)?;
        let state = stage.path().join("state.bin");
        let memory = stage.path().join("memory.bin");
        self.snapshot_create(
            &row,
            state.to_str().context("invalid snapshot path")?,
            memory.to_str().context("invalid memory path")?,
        )
        .await?;
        let mut files = BTreeMap::new();
        let empty = || SnapshotFile {
            size_bytes: 0,
            sha256: String::new(),
        };
        for name in ["state.bin", "memory.bin"] {
            files.insert(name.into(), empty());
        }
        let mut total = std::fs::metadata(&state)?
            .len()
            .checked_add(std::fs::metadata(&memory)?.len())
            .context("snapshot size overflow")?;
        for name in &names {
            total = total
                .checked_add(
                    firemage_snapshots::open_private(&self.directory(id).join(name))?
                        .metadata()?
                        .len(),
                )
                .context("snapshot size overflow")?;
        }
        anyhow::ensure!(
            total <= self.config.snapshot_max_bytes(),
            "snapshot exceeds configured size limit"
        );
        let source = self.directory(id);
        let target = stage.path().to_owned();
        let copy_names = names.clone();
        tokio::task::spawn_blocking(move || {
            for name in copy_names {
                let mut input = firemage_snapshots::open_private(&source.join(&name))?;
                let mut output = firemage_snapshots::create_private(&target.join(name))?;
                std::io::copy(&mut input, &mut output)?;
                output.sync_all()?;
            }
            anyhow::Ok(())
        })
        .await??;
        for name in names {
            files.insert(name, empty());
        }
        let network: Option<firemage_wire::NetworkSpec> = if let Some(net) = &spec.network {
            Some(serde_json::from_str(
                &firemage_queries::network(&self.db, owner, &net.network)
                    .await?
                    .spec,
            )?)
        } else {
            None
        };
        let gateway_mac = if spec.network.is_some() {
            Some(firemage_network::current_tap_mac(id).await?)
        } else {
            None
        };
        let mut spec = spec;
        if let (Some(attachment), Some(definition)) = (&mut spec.network, &network) {
            attachment.network = definition.name.clone();
        }
        let manifest = SnapshotManifest {
            version: 1,
            source_vm_name: spec.name.clone(),
            architecture: std::env::consts::ARCH.into(),
            firecracker_version: version,
            spec,
            network,
            gateway_mac,
            files,
        };
        let archive = stage.path().join("bundle.fmsnap");
        let output = archive.clone();
        let source = stage.path().to_owned();
        let limit = self.config.snapshot_max_bytes();
        let manifest = tokio::task::spawn_blocking(move || {
            firemage_snapshots::pack(&source, manifest, &output, limit)
        })
        .await??;
        self.publish_snapshot(owner, alias, Some(id), true, &archive, manifest)
            .await
    }
}
