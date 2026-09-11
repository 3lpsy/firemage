use anyhow::Context;
use firemage_wire::{GuestFileEntry, ensure_guest_inode};
use std::path::Path;
use tokio::io::AsyncWrite;

pub async fn guest_directory(disk: &Path, inode: u32) -> anyhow::Result<Vec<GuestFileEntry>> {
    ensure_guest_inode(inode)?;
    ensure_type(disk, inode, "directory").await?;
    let mut output = Vec::new();
    execute(disk, &format!("ls -p <{inode}>"), &mut output, 1024 * 1024).await?;
    super::parse::directory(&output)
}

pub async fn guest_download(
    disk: &Path,
    inode: u32,
    output: &mut (impl AsyncWrite + Unpin),
    max_bytes: u64,
) -> anyhow::Result<u64> {
    ensure_guest_inode(inode)?;
    ensure_type(disk, inode, "regular").await?;
    execute(disk, &format!("cat <{inode}>"), output, max_bytes).await
}

async fn ensure_type(disk: &Path, inode: u32, kind: &str) -> anyhow::Result<()> {
    let mut output = Vec::new();
    execute(disk, &format!("stat <{inode}>"), &mut output, 64 * 1024).await?;
    super::parse::ensure_type(&output, inode, kind)
}

async fn execute(
    disk: &Path,
    request: &str,
    output: &mut (impl AsyncWrite + Unpin),
    max_bytes: u64,
) -> anyhow::Result<u64> {
    let mut sandbox = crate::extract_sandbox::command_request(disk, request)
        .await
        .context("guest browsing requires bubblewrap, e2fsprogs and enabled user namespaces")?;
    super::process::run(&mut sandbox.command, output, max_bytes).await
}
