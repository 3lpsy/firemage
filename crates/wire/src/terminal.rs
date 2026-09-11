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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct WebTerminal {
    pub command: Vec<String>,
}
impl Default for WebTerminal {
    fn default() -> Self {
        Self {
            command: vec!["/bin/sh".into(), "-i".into()],
        }
    }
}
impl WebTerminal {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.command.is_empty() && self.command.len() <= 64,
            "web terminal command requires 1-64 arguments"
        );
        anyhow::ensure!(
            self.command[0].starts_with('/') && self.command[0].len() > 1,
            "web terminal executable must be an absolute guest path"
        );
        anyhow::ensure!(
            self.command
                .iter()
                .all(|arg| arg.len() <= 4096 && !arg.contains('\0'))
                && self.command.iter().map(String::len).sum::<usize>() <= 16384,
            "web terminal arguments exceed limits or contain NUL"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebShellSession {
    pub url: String,
    pub ticket: String,
}
