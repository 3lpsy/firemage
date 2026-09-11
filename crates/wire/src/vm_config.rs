use crate::{AssetAttachment, VmSpec};
use serde::{Deserialize, Serialize};

/// Portable catalog references use aliases; resolved credentials are never included.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmConfigDocument {
    pub version: u32,
    pub kernel_alias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress_policy_alias: Option<String>,
    #[serde(default)]
    pub attachments: Vec<ConfigAttachment>,
    pub vm: VmSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigAttachment {
    pub alias: String,
    pub destination: String,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}
impl ConfigAttachment {
    pub fn resolve(&self, id: String) -> AssetAttachment {
        AssetAttachment {
            asset_id: id,
            destination: self.destination.clone(),
            uid: self.uid,
            gid: self.gid,
            mode: self.mode,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmConfigImport {
    pub toml: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VmConfigPreview {
    pub name: String,
    pub references: Vec<ConfigReference>,
    pub address: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ConfigReference {
    pub kind: String,
    pub alias: String,
}
