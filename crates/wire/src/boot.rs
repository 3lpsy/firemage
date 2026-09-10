use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootFile {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub encoding: FileEncoding,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default = "default_mode")]
    pub mode: u32,
}
fn default_mode() -> u32 {
    0o644
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileEncoding {
    #[default]
    Utf8,
    Base64,
}
impl BootFile {
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::ensure_guest_path(&self.path)?;
        anyhow::ensure!(
            self.mode <= 0o777,
            "boot file mode must contain only owner/group/other permissions"
        );
        anyhow::ensure!(
            self.uid < u32::MAX && self.gid < u32::MAX,
            "invalid guest file owner"
        );
        if let Some(destination) = &self.destination {
            anyhow::ensure!(
                destination.starts_with('/'),
                "boot file destination must be an absolute guest path"
            );
            crate::ensure_guest_path(&destination[1..])?;
            anyhow::ensure!(
                !["/dev", "/proc", "/sys", "/firemage/input"]
                    .iter()
                    .any(|reserved| destination == reserved
                        || destination.starts_with(&format!("{reserved}/"))),
                "boot file destination uses a reserved guest path"
            );
        }
        Ok(())
    }
}
