use crate::Runtime;
use anyhow::Context;
use firemage_wire::{IsolationMode, VmSpec};
use std::path::{Path, PathBuf};
impl Runtime {
    pub(crate) async fn materialize_assets(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
    ) -> anyhow::Result<()> {
        self.ensure_isolation_policy(spec)?;
        let mut resolved = spec.clone();
        let directory = self.directory(&row.id);
        let rootfs_exists = directory.join("rootfs.ext4").exists();
        for asset in resolved
            .kernel
            .iter_mut()
            .chain(resolved.rootfs.iter_mut().filter(|_| !rootfs_exists))
            .chain(resolved.initrd.iter_mut())
            .chain(
                resolved
                    .drives
                    .iter_mut()
                    .filter(|drive| !directory.join(format!("drive-{}.img", drive.id)).exists())
                    .map(|drive| &mut drive.asset),
            )
        {
            if let firemage_wire::Asset::Local { path } = asset {
                *path = super::ensure_allowed_path(
                    path,
                    self.config.local_asset_roots.as_deref().unwrap_or_default(),
                    true,
                )?;
                super::ensure_trusted_path(path)?;
            }
        }
        let spec = &resolved;
        self.validate_dependencies(&row.owner_id, spec).await?;
        let dir = self.directory(&row.id);
        tokio::fs::create_dir_all(&dir).await?;
        let kernel = spec
            .kernel
            .as_ref()
            .context("kernel required for prepare/start")?;
        anyhow::ensure!(
            !matches!(kernel, firemage_wire::Asset::Oci { .. }),
            "kernel must be a local or remote binary"
        );
        firemage_assets::materialize(kernel, &dir.join("kernel")).await?;
        if !dir.join("rootfs.ext4").exists() {
            firemage_assets::materialize(
                spec.rootfs
                    .as_ref()
                    .context("rootfs required for prepare/start")?,
                &dir.join("rootfs.ext4"),
            )
            .await?;
        }
        if let Some(initrd) = &spec.initrd {
            firemage_assets::materialize(initrd, &dir.join("initrd")).await?;
        }
        for drive in &spec.drives {
            let path = dir.join(format!("drive-{}.img", drive.id));
            if !path.exists() {
                firemage_assets::materialize(&drive.asset, &path).await?;
            }
        }
        let files = self.seed_files(&row.owner_id, spec).await?;
        firemage_assets::seed(&files, spec.userdata.as_deref(), &dir).await?;
        Ok(())
    }
    pub(crate) async fn resource_path(
        &self,
        row: &firemage_orm::vms::Model,
        name: &str,
        writable: bool,
    ) -> anyhow::Result<PathBuf> {
        let source = self.directory(&row.id).join(name);
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.security.mode != IsolationMode::Jailed {
            return Ok(source);
        }
        let destination = self.jail_root(row).join("resources").join(name);
        if destination.exists() {
            tokio::fs::remove_file(&destination).await?;
        }
        anyhow::ensure!(
            tokio::fs::symlink_metadata(&source).await?.is_file(),
            "VM asset must be a regular file"
        );
        tokio::fs::hard_link(&source, &destination)
            .await
            .context("jail resources must share the VM data filesystem")?;
        super::own(
            &destination,
            self.jail_uid(&row.id)?,
            if writable { 0o600 } else { 0o400 },
        )?;
        Ok(Path::new("/resources").join(name))
    }
    pub(crate) async fn jail_file_limit(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
    ) -> anyhow::Result<u64> {
        let mut minimum = (u64::from(spec.memory_mib) + 64) * 1024 * 1024;
        for name in std::iter::once("rootfs.ext4".to_owned()).chain(
            spec.drives
                .iter()
                .filter(|d| !d.read_only)
                .map(|d| format!("drive-{}.img", d.id)),
        ) {
            minimum = minimum.max(
                tokio::fs::metadata(self.directory(&row.id).join(name))
                    .await?
                    .len(),
            );
        }
        let limit = spec
            .security
            .file_size_mib
            .map(|v| v * 1024 * 1024)
            .unwrap_or(minimum);
        anyhow::ensure!(
            limit >= minimum,
            "file_size_mib must cover writable disk sizes and guest memory plus 64 MiB for snapshots"
        );
        Ok(limit)
    }
}
