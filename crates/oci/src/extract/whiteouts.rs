use super::{
    filesystem::{Root, remove},
    metadata::ExtractionBudget,
};
use anyhow::{Result, ensure};
use rustix::{fs, io::Errno};
use std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) enum Whiteout {
    Remove(PathBuf),
    Opaque(PathBuf),
}

pub(super) fn parse(path: &Path) -> Result<Option<Whiteout>> {
    let Some(name) = path.file_name() else {
        return Ok(None);
    };
    let Some(target) = name.as_bytes().strip_prefix(b".wh.") else {
        return Ok(None);
    };
    let parent = path.parent().unwrap_or(Path::new(""));
    if target == b".wh..opq" {
        return Ok(Some(Whiteout::Opaque(parent.to_owned())));
    }
    ensure!(
        !target.is_empty() && target != b"." && target != b".." && !target.starts_with(b".wh."),
        "invalid OCI whiteout target"
    );
    Ok(Some(Whiteout::Remove(
        parent.join(OsStr::from_bytes(target)),
    )))
}

pub(super) fn apply(root: &Root, whiteout: Whiteout, budget: &mut ExtractionBudget) -> Result<()> {
    let result = match whiteout {
        Whiteout::Remove(path) => root.location(&path, false).and_then(|location| {
            remove(&location)?;
            budget.forget(&location.path);
            Ok(())
        }),
        Whiteout::Opaque(path) => root.directory(&path, false).and_then(|(fd, physical)| {
            for entry in fs::Dir::read_from(&fd)? {
                let entry = entry?;
                let name = OsStr::from_bytes(entry.file_name().to_bytes());
                if name == "." || name == ".." {
                    continue;
                }
                let location = root.location(&physical.join(name), false)?;
                remove(&location)?;
                budget.forget(&location.path);
            }
            Ok(())
        }),
    };
    match result {
        Err(error) if error.downcast_ref::<Errno>() == Some(&Errno::NOENT) => Ok(()),
        other => other,
    }
}
