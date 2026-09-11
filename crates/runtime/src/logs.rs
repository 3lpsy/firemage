use std::{fs::File, os::unix::fs::OpenOptionsExt, path::Path};

pub(crate) struct ProcessLogs {
    pub serial: File,
    pub stderr: File,
}

impl ProcessLogs {
    pub fn open(directory: &Path) -> anyhow::Result<Self> {
        let _logger = open(&directory.join("firecracker.log"), true)?;
        Ok(Self {
            serial: open(&directory.join("serial.log"), false)?,
            stderr: open(&directory.join("firecracker-stderr.log"), true)?,
        })
    }
}

fn open(path: &Path, truncate: bool) -> anyhow::Result<File> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!truncate)
        .truncate(truncate)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    anyhow::ensure!(file.metadata()?.is_file(), "VM log must be a regular file");
    Ok(file)
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
