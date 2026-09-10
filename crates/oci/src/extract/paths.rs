use anyhow::{Result, bail, ensure};
use std::{
    ffi::OsString,
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
};

pub(super) fn archive_path(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.as_os_str().as_bytes().len() <= 4096,
        "OCI path exceeds 4096 bytes"
    );
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => {
                ensure!(!name.as_bytes().contains(&0), "OCI path contains NUL");
                clean.push(name);
            }
            _ => bail!("OCI archive path must be relative without parent traversal"),
        }
    }
    ensure!(
        clean.components().count() <= 128,
        "OCI path exceeds 128 components"
    );
    Ok(clean)
}

pub(super) fn link_components(path: &Path) -> Result<Vec<OsString>> {
    ensure!(
        path.as_os_str().as_bytes().len() <= 4096,
        "OCI link exceeds 4096 bytes"
    );
    ensure!(
        !path.as_os_str().as_bytes().contains(&0),
        "OCI link contains NUL"
    );
    ensure!(!path.as_os_str().is_empty(), "OCI link target is empty");
    let components: Vec<_> = path
        .components()
        .filter_map(|part| match part {
            Component::RootDir | Component::CurDir => None,
            Component::ParentDir => Some(OsString::from("..")),
            Component::Normal(name) => Some(name.to_owned()),
            Component::Prefix(_) => None,
        })
        .collect();
    ensure!(components.len() <= 128, "OCI link exceeds 128 components");
    Ok(components)
}
