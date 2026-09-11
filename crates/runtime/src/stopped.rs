use crate::Runtime;
impl Runtime {
    pub(crate) async fn ensure_stopped_process(
        &self,
        row: &firemage_orm::vms::Model,
    ) -> anyhow::Result<()> {
        let mut children = self.children.lock().await;
        if let Some(child) = children.get_mut(&row.id) {
            anyhow::ensure!(
                child.try_wait()?.is_some(),
                "VM process is still running; stop it before accessing its disks"
            );
        }
        drop(children);
        anyhow::ensure!(
            !row.pid
                .zip(row.process_start.as_deref())
                .is_some_and(|(pid, start)| crate::process::is_alive(pid, start)),
            "VM process is still running; stop it before accessing its disks"
        );
        match tokio::net::UnixStream::connect(&row.socket).await {
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(())
            }
            Err(error) => Err(error.into()),
            Ok(_) => anyhow::bail!("VM socket is still active; stop it before accessing its disks"),
        }
    }
}

#[cfg(test)]
#[path = "stopped_tests.rs"]
mod tests;
