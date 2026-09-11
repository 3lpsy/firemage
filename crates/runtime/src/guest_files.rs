use crate::Runtime;
use firemage_wire::{GuestDirectory, VmSpec, ensure_guest_inode};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

impl Runtime {
    pub async fn guest_directory(
        &self,
        owner: &str,
        id: &str,
        inode: u32,
    ) -> anyhow::Result<GuestDirectory> {
        let _guard = self.lock(id).await;
        let _disk = self.guest_disk(owner, id, inode).await?;
        Ok(GuestDirectory {
            inode,
            entries: firemage_assets::guest_directory(
                &self.directory(id).join("rootfs.ext4"),
                inode,
            )
            .await?,
            max_file_bytes: self.config.asset_max_bytes(),
        })
    }

    pub async fn guest_download(
        &self,
        owner: &str,
        id: &str,
        inode: u32,
    ) -> anyhow::Result<(tokio::fs::File, u64)> {
        let _guard = self.lock(id).await;
        let _disk = self.guest_disk(owner, id, inode).await?;
        let mut file = tokio::fs::File::from_std(tempfile::tempfile_in(self.directory(id))?);
        let size = firemage_assets::guest_download(
            &self.directory(id).join("rootfs.ext4"),
            inode,
            &mut file,
            self.config.asset_max_bytes(),
        )
        .await?;
        file.flush().await?;
        file.rewind().await?;
        Ok((file, size))
    }

    async fn guest_disk(&self, owner: &str, id: &str, inode: u32) -> anyhow::Result<std::fs::File> {
        ensure_guest_inode(inode)?;
        let row = self
            .refresh(firemage_queries::vm(&self.db, owner, id).await?)
            .await?;
        anyhow::ensure!(
            row.state == "stopped",
            "guest browsing requires a stopped VM"
        );
        self.ensure_stopped_process(&row).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        let path = self.directory(id).join("rootfs.ext4");
        if spec.security.mode == firemage_wire::IsolationMode::Jailed {
            crate::output_disk::claim_stopped_disk(&path)
        } else {
            use std::os::unix::fs::OpenOptionsExt;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)?;
            anyhow::ensure!(
                file.metadata()?.is_file(),
                "guest disk must be a regular file"
            );
            Ok(file)
        }
    }
}

#[cfg(test)]
#[path = "guest_file_tests.rs"]
mod tests;
