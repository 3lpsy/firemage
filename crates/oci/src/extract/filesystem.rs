use super::metadata::ExtractionBudget;
use super::paths::link_components;
use anyhow::{Context, Result, bail, ensure};
use rustix::{
    fd::OwnedFd,
    fs::{self, AtFlags, FileType, Mode, OFlags, ResolveFlags},
    io::Errno,
};
use std::{
    cell::Cell,
    collections::VecDeque,
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) struct Root {
    pub fd: OwnedFd,
    pub nodes: Cell<u64>,
    max_nodes: u64,
}

pub(super) struct Location {
    pub parent: OwnedFd,
    pub name: OsString,
    pub path: PathBuf,
}

impl Root {
    pub fn open(path: &Path) -> Result<Self> {
        let fd = fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let stat = fs::fstat(&fd)?;
        ensure!(
            stat.st_mode & 0o077 == 0,
            "OCI staging root must be private (0700)"
        );
        ensure!(
            stat.st_uid == rustix::process::geteuid().as_raw(),
            "OCI staging root must be owned by the importer"
        );
        Ok(Self {
            fd,
            nodes: Cell::new(0),
            max_nodes: 0,
        })
    }

    pub fn with_budget(path: &Path, budget: &ExtractionBudget) -> Result<Self> {
        let mut root = Self::open(path)?;
        root.nodes.set(budget.nodes);
        root.max_nodes = budget.max_entries;
        Ok(root)
    }

    pub fn reserve_node(&self) -> Result<()> {
        let nodes = self
            .nodes
            .get()
            .checked_add(1)
            .context("OCI staging node accounting overflow")?;
        ensure!(nodes <= self.max_nodes, "OCI staging node limit exceeded");
        self.nodes.set(nodes);
        Ok(())
    }

    fn root_fd(&self) -> Result<OwnedFd> {
        Ok(fs::openat(
            &self.fd,
            ".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
    }

    pub fn directory(&self, path: &Path, create: bool) -> Result<(OwnedFd, PathBuf)> {
        let mut pending: VecDeque<_> = link_components(if path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            path
        })?
        .into();
        let mut fd = self.root_fd()?;
        let mut physical = PathBuf::new();
        let mut followed = 0;
        while let Some(name) = pending.pop_front() {
            if name == ".." {
                ensure!(physical.pop(), "OCI symlink traverses outside guest root");
                fd = fs::openat2(
                    &self.fd,
                    if physical.as_os_str().is_empty() {
                        Path::new(".")
                    } else {
                        &physical
                    },
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
                )?;
                continue;
            }
            let stat = match fs::statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => stat,
                Err(Errno::NOENT) if create => {
                    self.reserve_node()?;
                    fs::mkdirat(&fd, &name, Mode::from_raw_mode(0o700))?;
                    fs::statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW)?
                }
                Err(error) => return Err(error.into()),
            };
            match FileType::from_raw_mode(stat.st_mode) {
                FileType::Directory => {
                    fd = fs::openat(
                        &fd,
                        &name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )?;
                    physical.push(&name);
                }
                FileType::Symlink => {
                    followed += 1;
                    ensure!(followed <= 40, "OCI symlink chain exceeds 40 links");
                    let target = fs::readlinkat(&fd, &name, Vec::new())?;
                    let target = Path::new(OsStr::from_bytes(target.to_bytes()));
                    let components = link_components(target)?;
                    if target.is_absolute() {
                        fd = self.root_fd()?;
                        physical.clear();
                    }
                    for part in components.into_iter().rev() {
                        pending.push_front(part);
                    }
                    ensure!(
                        pending.len() + physical.components().count() <= 256,
                        "OCI resolved path is too deep"
                    );
                }
                _ => bail!("OCI path ancestor is not a directory"),
            }
        }
        Ok((fd, physical))
    }

    pub fn location(&self, path: &Path, create: bool) -> Result<Location> {
        let name = path
            .file_name()
            .context("OCI entry cannot replace guest root")?
            .to_owned();
        let (parent, mut physical) =
            self.directory(path.parent().unwrap_or(Path::new("")), create)?;
        physical.push(&name);
        Ok(Location {
            parent,
            name,
            path: physical,
        })
    }
}

pub(super) fn remove(location: &Location) -> Result<()> {
    remove_at(&location.parent, &location.name, 0)
}

fn remove_at(parent: &OwnedFd, name: &OsStr, depth: usize) -> Result<()> {
    ensure!(depth <= 256, "OCI deletion tree exceeds depth limit");
    let stat = match fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
        let fd = fs::openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        for entry in fs::Dir::read_from(&fd)? {
            let entry = entry?;
            let child = OsStr::from_bytes(entry.file_name().to_bytes());
            if child != "." && child != ".." {
                remove_at(&fd, child, depth + 1)?;
            }
        }
        fs::unlinkat(parent, name, AtFlags::REMOVEDIR)?;
    } else {
        fs::unlinkat(parent, name, AtFlags::empty())?;
    }
    Ok(())
}

pub(crate) fn has_file(root: &Path, path: &str) -> Result<bool> {
    let root = Root::open(root)?;
    match fs::openat2(
        &root.fd,
        path,
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
    ) {
        Ok(fd) => Ok(FileType::from_raw_mode(fs::fstat(fd)?.st_mode) == FileType::RegularFile),
        Err(Errno::NOENT | Errno::NOTDIR) => Ok(false),
        Err(error) => Err(error).context("resolve file inside OCI guest root"),
    }
}
