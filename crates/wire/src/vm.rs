use crate::{BootFile, NetworkAttachment};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Asset {
    Local {
        path: PathBuf,
    },
    Remote {
        url: String,
        sha256: String,
    },
    Oci {
        image: String,
        #[serde(default = "disk_size")]
        size_mib: u64,
    },
}
fn disk_size() -> u64 {
    2048
}
fn cpus() -> u8 {
    1
}
fn memory() -> u32 {
    256
}
fn boot_args() -> String {
    "console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmSpec {
    #[serde(default)]
    pub security: crate::VmSecurity,
    pub name: String,
    pub kernel: Option<Asset>,
    pub rootfs: Option<Asset>,
    pub initrd: Option<Asset>,
    #[serde(default = "cpus")]
    pub vcpus: u8,
    #[serde(default = "memory")]
    pub memory_mib: u32,
    #[serde(default = "boot_args")]
    pub boot_args: String,
    pub socket: Option<PathBuf>,
    pub network: Option<NetworkAttachment>,
    #[serde(default)]
    pub egress: Option<firemage_egress_policy::EgressPolicy>,
    #[serde(default)]
    pub environment: BTreeMap<String, crate::EnvironmentValue>,
    pub metadata: Option<Value>,
    pub userdata: Option<String>,
    #[serde(default)]
    pub files: Vec<BootFile>,
    #[serde(default)]
    pub drives: Vec<Drive>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drive {
    pub id: String,
    pub asset: Asset,
    #[serde(default)]
    pub read_only: bool,
}
impl VmSpec {
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::ensure_name(&self.name)?;
        self.security.validate(self.vcpus)?;
        anyhow::ensure!(
            self.socket.is_some() == (self.security.mode == crate::IsolationMode::External),
            "attached sockets require explicit external isolation mode; external mode requires a socket"
        );
        crate::validate_environment(&self.environment)?;
        if let Some(egress) = &self.egress {
            egress.validate()?;
            anyhow::ensure!(
                self.network.is_some() && self.socket.is_none(),
                "egress requires a managed VM and a Firemage-only network"
            );
        }
        anyhow::ensure!((1..=32).contains(&self.vcpus), "vcpus must be 1-32");
        anyhow::ensure!(
            (64..=1_048_576).contains(&self.memory_mib),
            "memory_mib must be 64-1048576"
        );
        anyhow::ensure!(
            self.boot_args.len() <= 4096 && !self.boot_args.contains('\0'),
            "invalid boot arguments"
        );
        anyhow::ensure!(
            self.metadata.is_none() || self.network.is_some(),
            "MMDS requires a network interface; use an isolated network"
        );
        anyhow::ensure!(
            self.files.len() <= 256,
            "at most 256 boot files are allowed"
        );
        anyhow::ensure!(
            self.userdata
                .as_ref()
                .is_none_or(|data| data.len() <= 4 * 1024 * 1024),
            "userdata exceeds 4 MiB"
        );
        let mut paths = std::collections::HashSet::new();
        for file in &self.files {
            anyhow::ensure!(
                paths.insert(&file.path)
                    && file.path != "user-data"
                    && file.path != "firemage"
                    && !file.path.starts_with("firemage/"),
                "duplicate or reserved boot file path"
            );
            file.validate()?;
        }
        let mut ids = std::collections::HashSet::from(["rootfs", "seed"]);
        for drive in &self.drives {
            crate::ensure_name(&drive.id)?;
            anyhow::ensure!(ids.insert(&drive.id), "duplicate or reserved drive id");
        }
        if let Some(net) = &self.network {
            crate::ensure_name(&net.network)?;
            let parts: Vec<_> = net.mac.split(':').collect();
            anyhow::ensure!(
                parts.len() == 6
                    && parts
                        .iter()
                        .all(|p| p.len() == 2 && u8::from_str_radix(p, 16).is_ok())
                    && u8::from_str_radix(parts[0], 16)? & 1 == 0,
                "invalid unicast MAC address"
            );
        }
        Ok(())
    }
}
pub fn ensure_guest_path(path: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !path.is_empty()
            && path.len() <= 4096
            && !path.starts_with('/')
            && path.split('/').all(|p| !p.is_empty()
                && p != "."
                && p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))),
        "guest path must be relative, without traversal or special characters"
    );
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VmState {
    Defined,
    Ready,
    Starting,
    Running,
    Paused,
    Stopped,
    Failed,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vm {
    pub id: String,
    pub owner_id: String,
    pub spec: VmSpec,
    pub state: VmState,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum VmAction {
    Start,
    Prepare,
    Launch,
    Pause,
    Resume,
    Shutdown,
    Stop,
    Refresh,
    Snapshot {
        snapshot_path: String,
        memory_path: String,
    },
    Restore {
        snapshot_path: String,
        memory_path: String,
    },
    Metadata {
        value: Value,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRequest {
    pub method: String,
    pub path: String,
    pub body: Option<Value>,
}
#[derive(Serialize, Deserialize)]
pub struct RawResponse {
    pub status: u16,
    pub body: Value,
}
