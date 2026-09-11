use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkloadMode {
    OneShot,
    KeepAlive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workload {
    pub mode: WorkloadMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
}

impl Workload {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(command) = &self.command {
            anyhow::ensure!(
                !command.is_empty()
                    && command.len() <= 256
                    && !command[0].is_empty()
                    && command.iter().all(|arg| !arg.contains('\0'))
                    && command.iter().map(String::len).sum::<usize>() <= 65536,
                "workload command needs 1-256 arguments, a nonempty program, no NUL, and at most 64 KiB"
            );
        }
        Ok(())
    }

    pub fn is_seed_init_required(&self) -> bool {
        self.mode == WorkloadMode::KeepAlive || self.command.is_some()
    }
}
