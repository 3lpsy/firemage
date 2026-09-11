use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuestFileKind {
    File,
    Directory,
    Symlink,
    Special,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuestFileEntry {
    pub inode: u32,
    pub name: String,
    pub kind: GuestFileKind,
    pub size_bytes: Option<u64>,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuestDirectory {
    pub inode: u32,
    pub entries: Vec<GuestFileEntry>,
    pub max_file_bytes: u64,
}

pub fn ensure_guest_inode(inode: u32) -> anyhow::Result<()> {
    anyhow::ensure!(inode >= 2, "guest inode must be at least 2");
    Ok(())
}
