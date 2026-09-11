use crate::{NetworkSpec, VmSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub owner_id: String,
    pub alias: String,
    pub source_vm_name: String,
    pub source_vm_id: Option<String>,
    pub created_at: i64,
    pub size_bytes: u64,
    pub expanded_bytes: u64,
    pub trusted: bool,
    pub architecture: String,
    pub firecracker_version: String,
    pub vcpus: u8,
    pub memory_mib: u32,
    pub requirements: SnapshotRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRequirements {
    pub network: Option<crate::NetworkAttachment>,
    #[serde(default)]
    pub network_definition: Option<NetworkSpec>,
    pub initrd: bool,
    pub drives: Vec<SnapshotDriveRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDriveRequirement {
    pub id: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotManifest {
    pub version: u32,
    pub source_vm_name: String,
    pub architecture: String,
    pub firecracker_version: String,
    pub spec: VmSpec,
    pub network: Option<NetworkSpec>,
    pub gateway_mac: Option<String>,
    pub files: BTreeMap<String, SnapshotFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFile {
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotUpload {
    pub alias: String,
    #[serde(default)]
    pub trusted: bool,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotSave {
    pub alias: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRestore {
    pub snapshot_id: String,
}
#[derive(Debug, Serialize)]
pub struct SnapshotLimits {
    pub max_bytes: u64,
}
