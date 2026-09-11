use crate::Runtime;
use anyhow::Context;
use firemage_wire::{TerminalInput, TerminalState, TerminalStatus, VmSpec};
use tokio::io::AsyncWriteExt;

impl Runtime {
    pub async fn terminal_status(&self, owner: &str, id: &str) -> anyhow::Result<TerminalStatus> {
        let _guard = self.lock(id).await;
        let row = firemage_queries::vm(&self.db, owner, id).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        let state = if !spec.terminal {
            TerminalState::Disabled
        } else if row.state != "running" {
            TerminalState::NotRunning
        } else {
            let mut children = self.children.lock().await;
            if let Some(child) = children.get_mut(id) {
                if child.try_wait()?.is_some() {
                    TerminalState::NotRunning
                } else if child.stdin.is_some() {
                    TerminalState::Available
                } else {
                    TerminalState::RestartRequired
                }
            } else {
                TerminalState::RestartRequired
            }
        };
        Ok(TerminalStatus { state })
    }

    // Only the assigned child's stdin is writable; input never names a host path or command.
    pub async fn terminal_input(
        &self,
        owner: &str,
        id: &str,
        input: &TerminalInput,
    ) -> anyhow::Result<()> {
        input.validate()?;
        let _guard = self.lock(id).await;
        let row = firemage_queries::vm(&self.db, owner, id).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        anyhow::ensure!(
            spec.terminal && spec.socket.is_none(),
            "terminal is disabled"
        );
        anyhow::ensure!(row.state == "running", "terminal requires a running VM");
        let mut children = self.children.lock().await;
        let child = children
            .get_mut(id)
            .context("terminal unavailable after server restart; stop and start the VM")?;
        anyhow::ensure!(child.try_wait()?.is_none(), "VM process has stopped");
        let stdin = child.stdin.as_mut().context("VM has no terminal input")?;
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            stdin.write_all(input.input.as_bytes()),
        )
        .await
        .context("terminal input timed out; some input may have been delivered")??;
        Ok(())
    }
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
