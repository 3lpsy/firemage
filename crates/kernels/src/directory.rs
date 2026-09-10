use firemage_wire::{KERNEL_MAX_BYTES, Kernel};
use std::{fs::File, path::Path};

pub struct Catalog(firemage_catalog_files::Directory);
impl Catalog {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self(firemage_catalog_files::Directory::open(
            path,
            1,
            KERNEL_MAX_BYTES,
        )?))
    }
    pub fn file(&self, name: &str) -> anyhow::Result<File> {
        self.0.file(name)
    }
    pub fn list(&self) -> anyhow::Result<Vec<Kernel>> {
        Ok(self
            .0
            .list()?
            .into_iter()
            .map(|entry| Kernel {
                name: entry.name,
                size_bytes: entry.size_bytes,
                alias: None,
                vm_count: 0,
            })
            .collect())
    }
    pub fn temporary(&self) -> anyhow::Result<tempfile::NamedTempFile> {
        self.0.temporary()
    }
    pub fn publish(&self, name: &str, file: tempfile::NamedTempFile) -> anyhow::Result<()> {
        self.0.publish(name, file)
    }
    pub fn upload(&self, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        self.0.upload(name, bytes)
    }
    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        self.0.remove(name)
    }
    pub fn copy(&self, name: &str, destination: &Path) -> anyhow::Result<()> {
        self.0.copy(name, destination)
    }
}
