use anyhow::Context;
use std::{
    fs::{File, Metadata, OpenOptions},
    io::ErrorKind,
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::net::{UnixListener, UnixStream};

pub(crate) struct SocketGuard {
    path: PathBuf,
    identity: Metadata,
    _lock: File,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if is_same_socket(&self.path, &self.identity) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

// The parent belongs to the service account; clients get traversal, never write access.
pub(crate) async fn bind(
    path: &Path,
    mode: u32,
    gid: Option<u32>,
) -> anyhow::Result<(UnixListener, SocketGuard)> {
    anyhow::ensure!(matches!(mode, 0o600 | 0o660), "invalid socket mode");
    anyhow::ensure!(
        mode != 0o660 || gid.is_some(),
        "shared socket requires a group"
    );
    let filename = path.file_name().context("socket path needs a filename")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = std::fs::canonicalize(parent)
        .context("create the socket directory before starting Firemage")?;
    let metadata = std::fs::metadata(&parent)?;
    // SAFETY: geteuid has no arguments or memory safety preconditions.
    let uid = unsafe { libc::geteuid() };
    anyhow::ensure!(
        metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o022 == 0,
        "socket directory must belong to the Firemage service user and must not be group/world writable"
    );
    let path = parent.join(filename);
    let mut lock_name = filename.to_os_string();
    lock_name.push(".lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent.join(lock_name))?;
    let lock_metadata = lock.metadata()?;
    anyhow::ensure!(
        lock_metadata.is_file()
            && lock_metadata.uid() == uid
            && lock_metadata.mode() & 0o077 == 0
            && lock_metadata.nlink() == 1,
        "socket lock must be a private file owned by the Firemage service user"
    );
    lock.try_lock()
        .context("another Firemage server owns this socket")?;
    remove_stale(&path, uid).await?;

    // Bind privately and publish the prepared inode without replacing any existing path.
    let staging = tempfile::Builder::new()
        .prefix(".fm-")
        .tempdir_in(&parent)?;
    let staged = staging.path().join("s");
    let listener = UnixListener::bind(&staged)?;
    if let Some(gid) = gid {
        std::os::unix::fs::chown(&staged, None, Some(gid)).context(
            "cannot assign unix_socket_gid; run with ownership privileges or a permitted group",
        )?;
    }
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(mode))?;
    let identity = std::fs::symlink_metadata(&staged)?;
    std::fs::hard_link(&staged, &path)
        .context("could not publish Unix socket without replacing its path")?;
    let guard = SocketGuard {
        path,
        identity,
        _lock: lock,
    };
    Ok((listener, guard))
}

async fn remove_stale(path: &Path, uid: u32) -> anyhow::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.file_type().is_socket() && metadata.uid() == uid,
        "refusing to replace a non-socket or socket owned by another user"
    );
    match tokio::time::timeout(Duration::from_millis(250), UnixStream::connect(path)).await {
        Ok(Err(error)) if error.kind() == ErrorKind::ConnectionRefused => {}
        _ => anyhow::bail!("socket is active or cannot safely be identified as stale"),
    }
    anyhow::ensure!(
        is_same_socket(path, &metadata),
        "socket changed during stale recovery"
    );
    std::fs::remove_file(path)?;
    Ok(())
}

fn is_same_socket(path: &Path, identity: &Metadata) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|current| {
        current.file_type().is_socket()
            && current.dev() == identity.dev()
            && current.ino() == identity.ino()
            && current.uid() == identity.uid()
    })
}
