use serde::{Deserialize, Serialize};

pub const TERMINAL_INPUT_MAX_BYTES: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalState {
    Disabled,
    NotRunning,
    Available,
    RestartRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalStatus {
    pub state: TerminalState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalInput {
    pub input: String,
}

impl TerminalInput {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.input.is_empty() && self.input.len() <= TERMINAL_INPUT_MAX_BYTES,
            "terminal input must contain 1-4096 bytes"
        );
        Ok(())
    }
}
