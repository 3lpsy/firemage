use anyhow::Context;
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
pub(crate) fn ensure_trusted_path(path: &Path) -> anyhow::Result<PathBuf> {
    let path = std::fs::canonicalize(path)?;
    for parent in path.ancestors() {
        let meta = std::fs::metadata(parent)?;
        anyhow::ensure!(
            meta.uid() == 0 && meta.permissions().mode() & 0o022 == 0,
            "jailer paths must be root-owned and not group/world writable: {}",
            parent.display()
        );
    }
    Ok(path)
}
pub(crate) fn executable(path: &Path) -> anyhow::Result<PathBuf> {
    let path = if path.components().count() == 1 {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join(path))
            .find(|p| p.is_file())
            .context("Firecracker or jailer binary not found in PATH")?
    } else {
        path.to_path_buf()
    };
    let path = ensure_trusted_path(&path)?;
    anyhow::ensure!(
        std::fs::metadata(&path)?.is_file(),
        "executable must be a regular file"
    );
    Ok(path)
}
pub(crate) fn own(path: &Path, uid: u32, mode: u32) -> anyhow::Result<()> {
    anyhow::ensure!(
        !std::fs::symlink_metadata(path)?.file_type().is_symlink(),
        "refusing symlink in jail"
    );
    std::os::unix::fs::chown(path, Some(uid), Some(uid))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

pub(crate) fn ensure_allowed_path(
    path: &Path,
    roots: &[PathBuf],
    strict: bool,
) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        path.is_absolute()
            && path
                .components()
                .all(|part| !matches!(part, std::path::Component::ParentDir)),
        "host asset/socket path must be absolute without traversal"
    );
    let canonical = match std::fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) if !strict && error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::canonicalize(path.parent().context("missing host path parent")?)?
                .join(path.file_name().context("missing host filename")?)
        }
        Err(error) => return Err(error.into()),
    };
    for root in roots {
        let root = std::fs::canonicalize(root)?;
        anyhow::ensure!(
            std::fs::metadata(&root)?.is_dir(),
            "allowed asset/socket root must be a directory"
        );
        if canonical.starts_with(&root) {
            if strict {
                for parent in canonical.ancestors() {
                    let metadata = std::fs::metadata(parent)?;
                    anyhow::ensure!(
                        metadata.uid() == 0 && metadata.mode() & 0o022 == 0,
                        "allowed host paths must be root-owned and not group/world writable: {}",
                        parent.display()
                    );
                    if parent == root {
                        break;
                    }
                }
            }
            return Ok(canonical);
        }
    }
    anyhow::bail!("host path is outside configured local_asset_roots or external_socket_roots")
}
