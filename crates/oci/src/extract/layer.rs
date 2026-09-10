use super::{
    archive,
    filesystem::{Root, remove},
    metadata::{ExtractionBudget, Metadata},
    paths::{archive_path, link_components},
    whiteouts,
};
use anyhow::{Context, Result, ensure};
use rustix::{
    fs::{self, AtFlags, FileType, Mode, OFlags},
    io::Errno,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, Seek},
    path::{Path, PathBuf},
};

pub(crate) fn apply_layer(
    root: &Path,
    archive: &Path,
    media_type: &str,
    expected_diff_id: &str,
    budget: &mut ExtractionBudget,
) -> Result<()> {
    let root = Root::with_budget(root, budget)?;
    let mut file = archive::spool(archive, media_type, expected_diff_id, budget)?;
    let mut whiteouts = Vec::new();
    let mut paths = BTreeSet::new();
    let mut path_bytes = 0;
    for item in tar::Archive::new(&mut file).entries_with_seek()? {
        let mut entry = item?;
        let path = validate(&mut entry)?;
        path_bytes += path.as_os_str().len() * 2 + 256;
        ensure!(
            path_bytes <= 128 * 1024 * 1024,
            "OCI layer path metadata exceeds 128 MiB"
        );
        ensure!(
            paths.insert(path.clone()),
            "OCI layer contains duplicate normalized paths"
        );
        if let Some(whiteout) = whiteouts::parse(&path)? {
            ensure!(
                entry.header().entry_type().is_file() && entry.size() == 0,
                "OCI whiteouts must be empty regular files"
            );
            whiteouts.push(whiteout);
        }
    }
    drop(paths);
    // Remove only the lower layer before installing any entries from this layer.
    for whiteout in whiteouts {
        whiteouts::apply(&root, whiteout, budget)?;
    }
    file.rewind()?;
    let mut pending = BTreeMap::new();
    for item in tar::Archive::new(&mut file).entries_with_seek()? {
        let mut entry = item?;
        let path = archive_path(&entry.path()?)?;
        if whiteouts::parse(&path)?.is_some() {
            continue;
        }
        install(&root, &mut entry, &path, budget, &mut pending)?;
    }
    let mut rounds = 0;
    while !pending.is_empty() {
        rounds += 1;
        ensure!(rounds <= 40, "OCI hardlink chain exceeds 40 links");
        let before = pending.len();
        let mut remaining = BTreeMap::new();
        for (path, target) in pending {
            if !hardlink(&root, &path, &target, budget)? {
                remaining.insert(path, target);
            }
        }
        ensure!(
            remaining.len() < before,
            "OCI hardlink target is missing or cyclic"
        );
        pending = remaining;
    }
    budget.nodes = root.nodes.get();
    Ok(())
}

fn validate<R: io::Read>(entry: &mut tar::Entry<'_, R>) -> Result<PathBuf> {
    let path = archive_path(&entry.path()?)?;
    let kind = entry.header().entry_type();
    ensure!(
        kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link(),
        "OCI archive contains unsupported special entry"
    );
    ensure!(
        !path.as_os_str().is_empty() || kind.is_dir(),
        "OCI entry cannot replace guest root"
    );
    Metadata::entry(entry)?;
    if kind.is_symlink() || kind.is_hard_link() {
        let target = entry.link_name()?.context("OCI link has no target")?;
        if kind.is_hard_link() {
            ensure!(
                !archive_path(&target)?.as_os_str().is_empty(),
                "OCI hardlink target is guest root"
            );
        } else {
            link_components(&target)?;
        }
        ensure!(entry.size() == 0, "OCI link cannot contain file data");
    }
    if let Some(extensions) = entry.pax_extensions()? {
        for extension in extensions {
            let extension = extension?;
            ensure!(
                !extension.key_bytes().starts_with(b"GNU.sparse"),
                "OCI sparse archives are not supported"
            );
        }
    }
    Ok(path)
}

fn install(
    root: &Root,
    entry: &mut tar::Entry<'_, &mut File>,
    path: &Path,
    budget: &mut ExtractionBudget,
    pending: &mut BTreeMap<PathBuf, PathBuf>,
) -> Result<()> {
    let metadata = Metadata::entry(entry)?;
    let kind = entry.header().entry_type();
    if path.as_os_str().is_empty() {
        budget.record(path.to_owned(), metadata)?;
        return Ok(());
    }
    let location = root.location(path, true)?;
    let existing = fs::statat(&location.parent, &location.name, AtFlags::SYMLINK_NOFOLLOW);
    let is_directory = existing
        .as_ref()
        .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::Directory);
    if !(kind.is_dir() && is_directory) {
        let invalidated: Vec<_> = pending
            .range(location.path.clone()..)
            .take_while(|(path, _)| path.starts_with(&location.path))
            .map(|(path, _)| path.clone())
            .collect();
        for path in invalidated {
            pending.remove(&path);
        }
        remove(&location)?;
        budget.forget(&location.path);
    }
    if kind.is_dir() {
        if !is_directory {
            root.reserve_node()?;
            fs::mkdirat(&location.parent, &location.name, Mode::from_raw_mode(0o700))?;
        }
    } else if kind.is_file() {
        root.reserve_node()?;
        let fd = fs::openat(
            &location.parent,
            &location.name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        let copied = io::copy(&mut *entry, &mut File::from(fd))?;
        ensure!(copied == entry.size(), "OCI regular file was truncated");
    } else if kind.is_symlink() {
        let target = entry.link_name()?.context("OCI symlink has no target")?;
        validate_symlink(&location.path, &target)?;
        root.reserve_node()?;
        fs::symlinkat(target.as_ref(), &location.parent, &location.name)?;
    } else {
        let target = archive_path(&entry.link_name()?.context("OCI hardlink has no target")?)?;
        if !hardlink(root, &location.path, &target, budget)? {
            ensure!(
                pending.len() < 65536,
                "OCI layer exceeds 65536 unresolved hardlinks"
            );
            pending.insert(location.path, target);
        }
        return Ok(());
    }
    budget.record(location.path, metadata)?;
    Ok(())
}

fn validate_symlink(path: &Path, target: &Path) -> Result<()> {
    let mut depth = if target.is_absolute() {
        0
    } else {
        path.parent().unwrap_or(Path::new("")).components().count()
    };
    for part in link_components(target)? {
        if part == ".." {
            ensure!(depth > 0, "OCI symlink traverses outside guest root");
            depth -= 1;
        } else {
            depth += 1;
        }
    }
    Ok(())
}
fn hardlink(
    root: &Root,
    path: &Path,
    target: &Path,
    budget: &mut ExtractionBudget,
) -> Result<bool> {
    let source = match root.location(target, false) {
        Ok(source) => source,
        Err(error) if error.downcast_ref::<Errno>() == Some(&Errno::NOENT) => return Ok(false),
        Err(error) => return Err(error),
    };
    let stat = match fs::statat(&source.parent, &source.name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile,
        "OCI hardlink target must be a regular file"
    );
    let destination = root.location(path, true)?;
    root.reserve_node()?;
    fs::linkat(
        &source.parent,
        &source.name,
        &destination.parent,
        &destination.name,
        AtFlags::empty(),
    )?;
    if let Some(metadata) = budget.metadata.get(&source.path).cloned() {
        budget.record(destination.path, metadata)?;
    }
    Ok(true)
}
