use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretAttachment {
    pub secret: String,
    pub destination: String,
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default = "private_mode")]
    pub mode: u32,
}

fn private_mode() -> u32 {
    0o600
}

impl SecretAttachment {
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::ensure_name(&self.secret)?;
        crate::BootFile {
            path: "secret".into(),
            content: String::new(),
            encoding: crate::FileEncoding::Utf8,
            destination: Some(self.destination.clone()),
            uid: self.uid,
            gid: self.gid,
            mode: self.mode,
        }
        .validate()
    }
}
