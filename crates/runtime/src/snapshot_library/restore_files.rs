use anyhow::Context;
use firemage_wire::SnapshotManifest;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

// The backup survives cancellation or failed process cleanup until an explicit rollback/commit.
pub(super) struct DiskSwap {
    directory: PathBuf,
    pub backup: PathBuf,
    old: Vec<String>,
    new: Vec<String>,
}
impl DiskSwap {
    pub fn install(
        directory: &Path,
        staged: &Path,
        manifest: &SnapshotManifest,
    ) -> anyhow::Result<Self> {
        let names: BTreeSet<String> = manifest
            .files
            .keys()
            .filter(|name| !matches!(name.as_str(), "state.bin" | "memory.bin"))
            .cloned()
            .collect();
        let mut managed = names.clone();
        managed.extend([
            "initrd".into(),
            "seed.ext4".into(),
            "oci-init-version".into(),
            "egress-bootstrap".into(),
        ]);
        for name in &managed {
            if let Ok(metadata) = std::fs::symlink_metadata(directory.join(name)) {
                anyhow::ensure!(
                    metadata.is_file(),
                    "existing VM resource is not a regular file: {name}"
                );
            }
        }
        let backup = tempfile::Builder::new()
            .prefix("restore-backup-")
            .tempdir_in(directory)?
            .keep();
        let mut swap = Self {
            directory: directory.into(),
            backup,
            old: Vec::new(),
            new: Vec::new(),
        };
        let result = (|| {
            for name in managed {
                let source = directory.join(&name);
                if source.try_exists()? {
                    std::fs::rename(source, swap.backup.join(&name))?;
                    swap.old.push(name);
                }
            }
            for name in names {
                std::fs::rename(staged.join(&name), directory.join(&name))?;
                swap.new.push(name);
            }
            anyhow::Ok(())
        })();
        if let Err(error) = result {
            swap.rollback()
                .context("restoring VM resources after installation failure")?;
            return Err(error);
        }
        Ok(swap)
    }
    pub fn rollback(&mut self) -> anyhow::Result<()> {
        for name in &self.new {
            match std::fs::remove_file(self.directory.join(name)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        self.new.clear();
        while let Some(name) = self.old.last() {
            std::fs::rename(self.backup.join(name), self.directory.join(name))?;
            self.old.pop();
        }
        std::fs::remove_dir(&self.backup)?;
        Ok(())
    }
    pub fn commit(self) -> anyhow::Result<()> {
        std::fs::remove_dir_all(self.backup)?;
        Ok(())
    }
}

pub(super) fn preserve_diagnostics(directory: &Path) -> anyhow::Result<()> {
    let suffix = uuid::Uuid::new_v4();
    for name in ["firecracker.log", "firecracker-stderr.log"] {
        let path = directory.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                anyhow::ensure!(metadata.is_file(), "VM diagnostics must be regular files");
                std::fs::rename(
                    path,
                    directory.join(format!("{name}.before-restore-{suffix}")),
                )?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
