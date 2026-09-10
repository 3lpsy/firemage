use crate::Runtime;
use anyhow::Context;
use firemage_wire::{IsolationMode, VmSpec};
use std::path::Path;
impl Runtime {
    pub(crate) async fn snapshot_paths(
        &self,
        row: &firemage_orm::vms::Model,
        state: &str,
        memory: &str,
    ) -> anyhow::Result<(String, String)> {
        let base = self.directory(&row.id).join("snapshots");
        tokio::fs::create_dir_all(&base).await?;
        anyhow::ensure!(
            !tokio::fs::symlink_metadata(&base)
                .await?
                .file_type()
                .is_symlink(),
            "managed snapshots directory cannot be a symlink"
        );
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        let base = if spec.security.mode == IsolationMode::Jailed {
            super::ensure_trusted_path(&base)?
        } else {
            tokio::fs::canonicalize(&base).await?
        };
        let state = resolve_path(&base, state)?;
        let memory = resolve_path(&base, memory)?;
        anyhow::ensure!(
            state != memory,
            "snapshot state and memory paths must differ"
        );
        Ok((state, memory))
    }
    pub(crate) async fn snapshot_create(
        &self,
        row: &firemage_orm::vms::Model,
        state: &str,
        memory: &str,
    ) -> anyhow::Result<()> {
        let fc = firemage_firecracker::Firecracker::new(Path::new(&row.socket))?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.security.mode != IsolationMode::Jailed {
            fc.call("PUT", "/snapshot/create", serde_json::json!({"snapshot_type":"Full","snapshot_path":state,"mem_file_path":memory})).await?;
            return Ok(());
        }
        let name = uuid::Uuid::new_v4().to_string();
        let guest_state = format!("/snapshots/{name}.state");
        let guest_memory = format!("/snapshots/{name}.memory");
        fc.call("PUT", "/snapshot/create", serde_json::json!({"snapshot_type":"Full","snapshot_path":guest_state,"mem_file_path":guest_memory})).await?;
        for (source, destination) in [(guest_state, state), (guest_memory, memory)] {
            let source = self.jail_root(row).join(source.trim_start_matches('/'));
            copy_private(&source, Path::new(destination)).await?;
            tokio::fs::remove_file(source).await?;
        }
        Ok(())
    }
    pub(crate) async fn snapshot_import(
        &self,
        row: &firemage_orm::vms::Model,
        state: &str,
        memory: &str,
    ) -> anyhow::Result<(String, String)> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.security.mode != IsolationMode::Jailed {
            return Ok((state.into(), memory.into()));
        }
        let mut paths = Vec::new();
        for (source, name) in [(state, "restore.state"), (memory, "restore.memory")] {
            let destination = self.jail_root(row).join("snapshots").join(name);
            copy_private(Path::new(source), &destination).await?;
            super::own(&destination, self.jail_uid(&row.id)?, 0o400)?;
            paths.push(format!("/snapshots/{name}"));
        }
        Ok((paths.remove(0), paths.remove(0)))
    }
    pub(crate) async fn stage_jail_resources(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
    ) -> anyhow::Result<()> {
        self.resource_path(row, "kernel", false).await?;
        self.resource_path(row, "rootfs.ext4", true).await?;
        if spec.initrd.is_some() {
            self.resource_path(row, "initrd", false).await?;
        }
        if self.directory(&row.id).join("seed.ext4").exists() {
            self.resource_path(row, "seed.ext4", false).await?;
        }
        for drive in &spec.drives {
            self.resource_path(row, &format!("drive-{}.img", drive.id), !drive.read_only)
                .await?;
        }
        Ok(())
    }
}
async fn copy_private(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let mut input = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(source)
        .await?;
    anyhow::ensure!(
        input.metadata().await?.is_file(),
        "snapshot must be a regular file"
    );
    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .await
        .context("snapshot destination must not exist")?;
    tokio::io::copy(&mut input, &mut output).await?;
    output.sync_all().await?;
    Ok(())
}

pub(super) fn resolve_path(base: &Path, value: &str) -> anyhow::Result<String> {
    let path = Path::new(value);
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .context("invalid snapshot filename")?;
    firemage_wire::ensure_name(
        name.strip_suffix(".json")
            .unwrap_or(name)
            .split('.')
            .next()
            .unwrap_or_default(),
    )?;
    anyhow::ensure!(
        name.len() <= 128
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            && name != "."
            && name != "..",
        "invalid snapshot filename"
    );
    if path.is_absolute() {
        anyhow::ensure!(
            path.parent() == Some(base),
            "snapshots require a filename or this VM's managed snapshots directory"
        );
    } else {
        anyhow::ensure!(
            path.components().count() == 1,
            "snapshot names cannot contain directories"
        );
    }
    let resolved = base.join(name);
    match std::fs::symlink_metadata(&resolved) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "snapshot path must be a regular file without symlinks"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(resolved.to_str().context("non-UTF8 snapshot path")?.into())
}
