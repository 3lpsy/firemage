use crate::Runtime;
use anyhow::Context;
use firemage_wire::{Snapshot, SnapshotManifest, SnapshotUpload};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

impl Runtime {
    pub fn snapshot_directory(&self) -> anyhow::Result<PathBuf> {
        let path = self.config.snapshot_dir();
        std::fs::create_dir_all(&path)?;
        anyhow::ensure!(
            std::fs::symlink_metadata(&path)?.is_dir(),
            "snapshot directory cannot be a symlink"
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(std::fs::canonicalize(path)?)
    }
    pub async fn snapshots(&self, owner: &str) -> anyhow::Result<Vec<Snapshot>> {
        firemage_queries::snapshots(&self.db, owner)
            .await?
            .into_iter()
            .map(snapshot_view)
            .collect()
    }
    pub async fn snapshots_all(&self) -> anyhow::Result<Vec<Snapshot>> {
        firemage_queries::all_snapshots(&self.db)
            .await?
            .into_iter()
            .map(snapshot_view)
            .collect()
    }
    pub async fn snapshot_by_id(&self, id: &str) -> anyhow::Result<Snapshot> {
        snapshot_view(firemage_queries::snapshot_by_id(&self.db, id).await?)
    }
    pub async fn snapshot(&self, owner: &str, id: &str) -> anyhow::Result<Snapshot> {
        snapshot_view(firemage_queries::snapshot(&self.db, owner, id).await?)
    }
    pub async fn import_snapshot(
        &self,
        owner: &str,
        input: SnapshotUpload,
        archive: &Path,
    ) -> anyhow::Result<Snapshot> {
        firemage_wire::ensure_asset_alias(&input.alias)?;
        let path = archive.to_owned();
        let limit = self.config.snapshot_max_bytes();
        let manifest =
            tokio::task::spawn_blocking(move || firemage_snapshots::inspect(&path, limit))
                .await??;
        self.publish_snapshot(owner, &input.alias, None, input.trusted, archive, manifest)
            .await
    }
    pub(super) async fn publish_snapshot(
        &self,
        owner: &str,
        alias: &str,
        source: Option<&str>,
        trusted: bool,
        archive: &Path,
        manifest: SnapshotManifest,
    ) -> anyhow::Result<Snapshot> {
        let _guard = self.lock("snapshot-library").await;
        firemage_wire::ensure_asset_alias(alias)?;
        anyhow::ensure!(
            !firemage_queries::snapshots(&self.db, owner)
                .await?
                .iter()
                .any(|row| row.alias == alias),
            "snapshot alias already exists"
        );
        let id = uuid::Uuid::new_v4().to_string();
        let target = self.snapshot_directory()?.join(format!("{id}.fmsnap"));
        let path = archive.to_owned();
        let limit = self.config.snapshot_max_bytes();
        let (size, sha256) =
            tokio::task::spawn_blocking(move || firemage_snapshots::hash_file(&path, limit))
                .await??;
        let expanded = manifest
            .files
            .values()
            .map(|file| file.size_bytes)
            .sum::<u64>();
        let row = firemage_orm::snapshots::Model {
            id: id.clone(),
            owner_id: owner.into(),
            alias: alias.into(),
            source_vm_id: source.map(str::to_owned),
            source_vm_name: manifest.source_vm_name.clone(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            size_bytes: size as i64,
            expanded_bytes: expanded as i64,
            sha256,
            trusted,
            manifest: serde_json::to_string(&manifest)?,
        };
        tokio::fs::rename(archive, &target).await?;
        if let Err(error) = firemage_queries::insert_snapshot(&self.db, row.clone()).await {
            let _ = tokio::fs::remove_file(target).await;
            return Err(error);
        }
        snapshot_view(row)
    }
    pub async fn trust_snapshot(&self, owner: &str, id: &str) -> anyhow::Result<Snapshot> {
        let _guard = self.lock("snapshot-library").await;
        firemage_queries::trust_snapshot(&self.db, owner, id).await?;
        self.snapshot(owner, id).await
    }
    pub async fn delete_snapshot(&self, owner: &str, id: &str) -> anyhow::Result<()> {
        let _guard = self.lock("snapshot-library").await;
        self.delete_snapshot_locked(owner, id).await
    }
    pub(super) async fn delete_snapshot_locked(&self, owner: &str, id: &str) -> anyhow::Result<()> {
        let row = firemage_queries::snapshot(&self.db, owner, id).await?;
        let path = self
            .snapshot_directory()?
            .join(format!("{}.fmsnap", row.id));
        match tokio::fs::remove_file(path).await {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
        firemage_queries::delete_snapshot(&self.db, owner, id).await
    }
    pub(crate) async fn delete_vm_snapshots(&self, owner: &str, vm: &str) -> anyhow::Result<()> {
        let _guard = self.lock("snapshot-library").await;
        for snapshot in firemage_queries::snapshots(&self.db, owner).await? {
            if snapshot.source_vm_id.as_deref() == Some(vm) {
                self.delete_snapshot_locked(owner, &snapshot.id).await?;
            }
        }
        Ok(())
    }
    pub async fn snapshot_download(
        &self,
        owner: &str,
        id: &str,
    ) -> anyhow::Result<(tokio::fs::File, Snapshot)> {
        let _guard = self.lock("snapshot-library").await;
        let row = firemage_queries::snapshot(&self.db, owner, id).await?;
        let file = firemage_snapshots::open_private(
            &self
                .snapshot_directory()?
                .join(format!("{}.fmsnap", row.id)),
        )?;
        anyhow::ensure!(
            file.metadata()?.len() == row.size_bytes as u64,
            "snapshot archive size changed"
        );
        Ok((tokio::fs::File::from_std(file), snapshot_view(row)?))
    }
    pub(super) async fn verified_snapshot(
        &self,
        owner: &str,
        id: &str,
        destination: &Path,
    ) -> anyhow::Result<SnapshotManifest> {
        let row = firemage_queries::snapshot(&self.db, owner, id).await?;
        anyhow::ensure!(
            row.trusted,
            "this uploaded snapshot must be trusted by an administrator before restore"
        );
        let path = self
            .snapshot_directory()?
            .join(format!("{}.fmsnap", row.id));
        let destination = destination.to_owned();
        let limit = self.config.snapshot_max_bytes();
        tokio::task::spawn_blocking(move || {
            let (_, hash) = firemage_snapshots::hash_file(&path, limit)?;
            anyhow::ensure!(
                hash == row.sha256,
                "snapshot archive failed integrity verification"
            );
            let manifest = firemage_snapshots::unpack(&path, &destination, limit)?;
            anyhow::ensure!(
                serde_json::to_string(&manifest)? == row.manifest,
                "snapshot manifest changed"
            );
            Ok(manifest)
        })
        .await
        .context("snapshot verification worker failed")?
    }
}
fn snapshot_view(row: firemage_orm::snapshots::Model) -> anyhow::Result<Snapshot> {
    let manifest: SnapshotManifest = serde_json::from_str(&row.manifest)?;
    Ok(Snapshot {
        id: row.id,
        owner_id: row.owner_id,
        alias: row.alias,
        source_vm_id: row.source_vm_id,
        source_vm_name: row.source_vm_name,
        created_at: row.created_at,
        size_bytes: row.size_bytes as u64,
        expanded_bytes: row.expanded_bytes as u64,
        trusted: row.trusted,
        architecture: manifest.architecture,
        firecracker_version: manifest.firecracker_version,
        vcpus: manifest.spec.vcpus,
        memory_mib: manifest.spec.memory_mib,
        requirements: firemage_wire::SnapshotRequirements {
            network: manifest.spec.network,
            network_definition: manifest.network,
            initrd: manifest.spec.initrd.is_some(),
            drives: manifest
                .spec
                .drives
                .into_iter()
                .map(|drive| firemage_wire::SnapshotDriveRequirement {
                    id: drive.id,
                    read_only: drive.read_only,
                })
                .collect(),
        },
    })
}
