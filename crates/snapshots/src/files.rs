use anyhow::Context;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

pub fn open_private(path: &Path) -> anyhow::Result<File> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "snapshot component must be a regular file"
    );
    Ok(file)
}
pub fn create_private(path: &Path) -> anyhow::Result<File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .context("snapshot output must not already exist")
}
pub fn hash_file(path: &Path, limit: u64) -> anyhow::Result<(u64, String)> {
    let mut file = open_private(path)?;
    anyhow::ensure!(
        file.metadata()?.len() <= limit,
        "snapshot exceeds configured size limit"
    );
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut bytes = [0; 65536];
    loop {
        let read = file.read(&mut bytes)?;
        if read == 0 {
            break;
        }
        count += read as u64;
        anyhow::ensure!(count <= limit, "snapshot exceeds configured size limit");
        hash.update(&bytes[..read]);
    }
    Ok((count, format!("{:x}", hash.finalize())))
}
