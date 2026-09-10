use super::{attributes, filesystem::Root};
use anyhow::{Context, Result, ensure};
use rustix::{
    fs::{self, AtFlags, FileType, Mode, OFlags, Timestamps, XattrFlags},
    process::{Gid, Uid},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(crate) struct ExtractionBudget {
    pub(super) max_bytes: u64,
    pub(super) max_entries: u64,
    pub(super) bytes: u64,
    pub(super) entries: u64,
    pub(super) nodes: u64,
    metadata_bytes: usize,
    pub(super) metadata: BTreeMap<PathBuf, Metadata>,
}

impl ExtractionBudget {
    pub(crate) fn new(max_bytes: u64, max_entries: u64) -> Self {
        Self {
            max_bytes,
            max_entries,
            bytes: 0,
            entries: 0,
            nodes: 0,
            metadata_bytes: 0,
            metadata: BTreeMap::new(),
        }
    }

    pub(super) fn forget(&mut self, path: &Path) {
        let keys: Vec<_> = self
            .metadata
            .range(path.to_owned()..)
            .take_while(|(entry, _)| entry.starts_with(path))
            .map(|(entry, _)| entry.clone())
            .collect();
        for key in keys {
            self.metadata.remove(&key);
        }
    }

    pub(super) fn record(&mut self, path: PathBuf, metadata: Metadata) -> Result<()> {
        let size = path.as_os_str().as_bytes().len()
            + 256
            + metadata
                .xattrs
                .iter()
                .map(|(name, value)| name.len() + value.len() + 64)
                .sum::<usize>();
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(size)
            .context("OCI metadata accounting overflow")?;
        ensure!(
            self.metadata_bytes <= 128 * 1024 * 1024,
            "OCI metadata exceeds 128 MiB budget"
        );
        self.metadata.insert(path, metadata);
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct Metadata {
    uid: u32,
    gid: u32,
    mode: u32,
    mtime: Option<Timestamps>,
    xattrs: Vec<(String, Vec<u8>)>,
}

impl Metadata {
    pub fn entry<R: std::io::Read>(entry: &mut tar::Entry<'_, R>) -> Result<Self> {
        let header = entry.header();
        let uid = u32::try_from(header.uid()?).context("OCI UID exceeds u32")?;
        let gid = u32::try_from(header.gid()?).context("OCI GID exceeds u32")?;
        ensure!(
            uid != u32::MAX && gid != u32::MAX,
            "OCI ownership uses reserved ID"
        );
        let mut metadata = Self {
            uid,
            gid,
            mode: header.mode()? & 0o7777,
            mtime: Some(attributes::timestamp(
                header.mtime()?.to_string().as_bytes(),
            )?),
            xattrs: Vec::new(),
        };
        let is_symlink = header.entry_type().is_symlink();
        let size = entry.size();
        if let Some(extensions) = entry.pax_extensions()? {
            for extension in extensions {
                let extension = extension?;
                match extension.key_bytes() {
                    b"uid" | b"gid" => {
                        let id = u32::try_from(attributes::decimal(extension.value_bytes())?)
                            .context("OCI PAX ownership exceeds u32")?;
                        ensure!(id != u32::MAX, "OCI PAX ownership uses reserved ID");
                        if extension.key_bytes() == b"uid" {
                            metadata.uid = id;
                        } else {
                            metadata.gid = id;
                        }
                    }
                    b"size" => ensure!(
                        attributes::decimal(extension.value_bytes())? == size,
                        "OCI PAX size disagrees with tar entry"
                    ),
                    _ => {}
                }
                if extension.key_bytes() == b"mtime" {
                    metadata.mtime = Some(attributes::timestamp(extension.value_bytes())?);
                }
                if let Some(name) = attributes::xattr_name(extension.key_bytes())? {
                    ensure!(!is_symlink, "OCI symlink xattrs are not supported");
                    ensure!(metadata.xattrs.len() < 64, "OCI entry exceeds 64 xattrs");
                    metadata
                        .xattrs
                        .push((name.to_owned(), extension.value_bytes().to_owned()));
                }
            }
        }
        Ok(metadata)
    }

    fn implicit(stat: &fs::Stat) -> Self {
        Self {
            uid: stat.st_uid,
            gid: stat.st_gid,
            mode: if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
                0o755
            } else {
                stat.st_mode & 0o7777
            },
            mtime: None,
            xattrs: Vec::new(),
        }
    }

    fn apply(&self, fd: &rustix::fd::OwnedFd) -> Result<()> {
        fs::fchown(
            fd,
            Some(Uid::from_raw(self.uid)),
            Some(Gid::from_raw(self.gid)),
        )?;
        fs::fchmod(fd, Mode::from_raw_mode(self.mode))?;
        if let Some(times) = &self.mtime {
            fs::futimens(fd, times)?;
        }
        for (name, value) in &self.xattrs {
            fs::fsetxattr(fd, name.as_str(), value, XattrFlags::empty())
                .with_context(|| format!("preserve OCI xattr {name}"))?;
        }
        Ok(())
    }
}

pub(crate) fn finalize(root: &Path, budget: &ExtractionBudget) -> Result<()> {
    let root = Root::open(root)?;
    visit(&root.fd, Path::new(""), budget, 0, &mut BTreeSet::new())?;
    Ok(())
}

fn visit(
    fd: &rustix::fd::OwnedFd,
    path: &Path,
    budget: &ExtractionBudget,
    depth: usize,
    seen: &mut BTreeSet<(u64, u64)>,
) -> Result<()> {
    ensure!(depth <= 256, "OCI metadata tree exceeds depth limit");
    for item in fs::Dir::read_from(fd)? {
        let item = item?;
        let name = OsStr::from_bytes(item.file_name().to_bytes());
        if name == "." || name == ".." {
            continue;
        }
        let child = path.join(name);
        let stat = fs::statat(fd, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let kind = FileType::from_raw_mode(stat.st_mode);
        if kind == FileType::RegularFile && !seen.insert((stat.st_dev, stat.st_ino)) {
            continue;
        }
        let metadata = budget
            .metadata
            .get(&child)
            .cloned()
            .unwrap_or_else(|| Metadata::implicit(&stat));
        if kind == FileType::Symlink {
            fs::chownat(
                fd,
                name,
                Some(Uid::from_raw(metadata.uid)),
                Some(Gid::from_raw(metadata.gid)),
                AtFlags::SYMLINK_NOFOLLOW,
            )?;
            if let Some(times) = &metadata.mtime {
                fs::utimensat(fd, name, times, AtFlags::SYMLINK_NOFOLLOW)?;
            }
        } else {
            ensure!(
                kind == FileType::RegularFile || kind == FileType::Directory,
                "unexpected special file in OCI staging tree"
            );
            let child_fd = fs::openat(
                fd,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )?;
            if kind == FileType::Directory {
                visit(&child_fd, &child, budget, depth + 1, seen)?;
            }
            metadata.apply(&child_fd)?;
        }
    }
    if path.as_os_str().is_empty() {
        let stat = fs::fstat(fd)?;
        let metadata = budget
            .metadata
            .get(path)
            .cloned()
            .unwrap_or_else(|| Metadata::implicit(&stat));
        metadata.apply(fd)?;
    }
    Ok(())
}
