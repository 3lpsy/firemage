use anyhow::Context;
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

// Keep a directory descriptor open so replacing a path cannot redirect file operations.
pub struct Directory {
    directory: File,
    maximum: u64,
    minimum: u64,
}
impl Directory {
    pub fn open(path: &Path, minimum: u64, maximum: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(
            path.is_absolute(),
            "catalog file directory must be absolute"
        );
        let mut directory = File::open("/")?;
        for component in path.components() {
            let Component::Normal(name) = component else {
                anyhow::ensure!(
                    component == Component::RootDir,
                    "invalid catalog file directory"
                );
                continue;
            };
            let name = CString::new(name.as_encoded_bytes())?;
            let fd = directory.as_raw_fd();
            // mkdirat and openat operate relative to the already checked parent.
            let result = unsafe { libc::mkdirat(fd, name.as_ptr(), 0o700) };
            if result != 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let next = unsafe {
                libc::openat(
                    fd,
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            anyhow::ensure!(
                next >= 0,
                "catalog file directory must contain no symlinks and be accessible"
            );
            directory = unsafe { File::from_raw_fd(next) };
        }
        Ok(Self {
            directory,
            minimum,
            maximum,
        })
    }
    pub(crate) fn maximum(&self) -> u64 {
        self.maximum
    }
    fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd()))
    }
    pub fn file(&self, name: &str) -> anyhow::Result<File> {
        ensure_filename(name)?;
        let name = CString::new(name)?;
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error()).context("catalog file is unavailable");
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        anyhow::ensure!(
            metadata.is_file() && metadata.nlink() == 1,
            "catalog file must be a regular file without symbolic or hard links"
        );
        anyhow::ensure!(
            metadata.len() >= self.minimum && metadata.len() <= self.maximum,
            "file size is outside catalog limits"
        );
        Ok(file)
    }
    pub fn list(&self) -> anyhow::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(self.path())? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if ensure_filename(&name).is_err() {
                continue;
            }
            if let Ok(file) = self.file(&name) {
                entries.push(Entry {
                    name,
                    size_bytes: file.metadata()?.len(),
                });
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
    pub fn temporary(&self) -> anyhow::Result<tempfile::NamedTempFile> {
        Ok(tempfile::Builder::new()
            .prefix(".import-")
            .tempfile_in(self.path())?)
    }
    pub fn publish(&self, name: &str, mut file: tempfile::NamedTempFile) -> anyhow::Result<()> {
        ensure_filename(name)?;
        anyhow::ensure!(
            file.as_file().metadata()?.len() >= self.minimum
                && file.as_file().metadata()?.len() <= self.maximum,
            "file size is outside catalog limits"
        );
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist_noclobber(self.path().join(name))
            .map_err(|e| {
                anyhow::anyhow!(
                    "cannot publish catalog file; name may already exist: {}",
                    e.error
                )
            })?;
        self.directory.sync_all()?;
        Ok(())
    }
    pub fn upload(&self, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        ensure_filename(name)?;
        anyhow::ensure!(
            bytes.len() as u64 >= self.minimum && bytes.len() as u64 <= self.maximum,
            "file size is outside catalog limits"
        );
        let mut file = self.temporary()?;
        file.write_all(bytes)?;
        self.publish(name, file)
    }
    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let _file = self.file(name)?;
        let name = CString::new(name)?;
        let result = unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) };
        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        self.directory.sync_all()?;
        Ok(())
    }
    pub fn copy(&self, name: &str, destination: &Path) -> anyhow::Result<()> {
        let input = self.file(name)?;
        let parent = destination
            .parent()
            .context("catalog file destination requires a parent")?;
        let mut output = tempfile::NamedTempFile::new_in(parent)?;
        let size = std::io::copy(&mut input.take(self.maximum + 1), &mut output)?;
        anyhow::ensure!(
            size >= self.minimum && size <= self.maximum,
            "catalog file exceeds size limit"
        );
        output.as_file().sync_all()?;
        output.persist(destination)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub size_bytes: u64,
}
fn ensure_filename(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 128
            && name.as_bytes()[0].is_ascii_alphanumeric()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "invalid catalog filename"
    );
    Ok(())
}
