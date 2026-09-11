use anyhow::Context;
use firemage_wire::BootFile;
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

// The seed is a guest disk; no directory from the host is mounted in a VM.
pub async fn seed(
    files: &[BootFile],
    userdata: Option<&str>,
    directory: &Path,
    maximum: u64,
) -> anyhow::Result<Option<PathBuf>> {
    let staging = directory.join("seed");
    match tokio::fs::symlink_metadata(&staging).await {
        Ok(meta) if meta.is_dir() => tokio::fs::remove_dir_all(&staging).await?,
        Ok(_) => tokio::fs::remove_file(&staging).await?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let disk = directory.join("seed.ext4");
    if files.is_empty() && userdata.is_none() {
        match tokio::fs::remove_file(&disk).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        return Ok(None);
    }
    let result = async {
        let bytes = stage(files, userdata, &staging, maximum).await?;
        let size_mib = 64.max(bytes.div_ceil(1024 * 1024) * 5 / 4 + 16);
        crate::ext4(&staging, &disk, size_mib, "firemage-seed").await?;
        tokio::fs::set_permissions(&disk, std::fs::Permissions::from_mode(0o600)).await?;
        anyhow::Ok(())
    }
    .await;
    if staging.exists() {
        tokio::fs::remove_dir_all(&staging).await?;
    }
    if result.is_err() {
        let _ = tokio::fs::remove_file(&disk).await;
    }
    result?;
    Ok(Some(disk))
}
async fn stage(
    files: &[BootFile],
    userdata: Option<&str>,
    staging: &Path,
    maximum: u64,
) -> anyhow::Result<u64> {
    private_directory(staging).await?;
    let mut setup = String::from("#!/bin/sh\nset -eu\n");
    let mut total = userdata.map_or(0, |value| value.len() as u64);
    anyhow::ensure!(
        total <= maximum,
        "combined guest boot inputs exceed {maximum} bytes"
    );
    for file in files {
        file.validate()?;
        anyhow::ensure!(
            file.path != "user-data" && file.path != "firemage/setup.sh",
            "reserved seed path"
        );
        let path = staging.join(&file.path);
        private_directory(path.parent().context("missing seed parent")?).await?;
        let bytes = match file.encoding {
            firemage_wire::FileEncoding::Utf8 => {
                std::borrow::Cow::Borrowed(file.content.as_bytes())
            }
            firemage_wire::FileEncoding::Base64 => {
                use base64::Engine;
                std::borrow::Cow::Owned(
                    base64::engine::general_purpose::STANDARD.decode(&file.content)?,
                )
            }
        };
        total = total
            .checked_add(bytes.len() as u64)
            .context("combined guest boot inputs are too large")?;
        anyhow::ensure!(
            total <= maximum,
            "combined guest boot inputs exceed {maximum} bytes"
        );
        private_file(&path, &bytes).await?;
        if let Some(destination) = &file.destination {
            let parent = Path::new(destination)
                .parent()
                .context("missing guest parent")?
                .to_str()
                .context("invalid guest parent")?;
            setup += &format!(
                "mkdir -p '{}'\ncp '/firemage/input/{}' '{}'\nchown {}:{} '{}'\nchmod {:o} '{}'\n",
                parent,
                file.path,
                destination,
                file.uid,
                file.gid,
                destination,
                file.mode,
                destination
            );
        }
    }
    if let Some(userdata) = userdata {
        private_file(&staging.join("user-data"), userdata.as_bytes()).await?;
    }
    private_directory(&staging.join("firemage")).await?;
    private_file(&staging.join("firemage/setup.sh"), setup.as_bytes()).await?;
    Ok(total)
}
async fn private_directory(path: &Path) -> anyhow::Result<()> {
    tokio::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .await?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    Ok(())
}
async fn private_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .await?;
    file.write_all(bytes).await?;
    file.flush().await?;
    Ok(())
}

#[cfg(test)]
#[path = "seed_tests.rs"]
mod tests;
