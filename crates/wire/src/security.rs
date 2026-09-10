use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IsolationMode {
    #[default]
    Jailed,
    Trusted,
    External,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VmSecurity {
    pub mode: IsolationMode,
    pub memory_overhead_mib: u32,
    pub pids_max: u32,
    pub cpu_percent: Option<u32>,
    pub file_size_mib: Option<u64>,
    pub open_files: u32,
}
impl Default for VmSecurity {
    fn default() -> Self {
        Self {
            mode: IsolationMode::Jailed,
            memory_overhead_mib: 256,
            pids_max: 128,
            cpu_percent: None,
            file_size_mib: None,
            open_files: 256,
        }
    }
}
impl VmSecurity {
    pub fn validate(&self, vcpus: u8) -> anyhow::Result<()> {
        anyhow::ensure!(
            (64..=65536).contains(&self.memory_overhead_mib),
            "memory_overhead_mib must be 64-65536"
        );
        anyhow::ensure!(
            (64..=4096).contains(&self.pids_max),
            "pids_max must be 64-4096"
        );
        anyhow::ensure!(
            (64..=4096).contains(&self.open_files),
            "open_files must be 64-4096"
        );
        anyhow::ensure!(
            self.cpu_percent
                .is_none_or(|v| (1..=u32::from(vcpus) * 100).contains(&v)),
            "cpu_percent must be 1 through vcpus * 100"
        );
        anyhow::ensure!(
            self.file_size_mib
                .is_none_or(|v| (64..=16_777_216).contains(&v)),
            "file_size_mib must be 64-16777216"
        );
        Ok(())
    }
}
