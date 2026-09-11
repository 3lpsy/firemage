use crate::Runtime;
use anyhow::Context;
use firemage_firecracker::Firecracker;
use firemage_wire::{IsolationMode, VmSpec};
impl Runtime {
    pub(crate) async fn launch(&self, row: &firemage_orm::vms::Model) -> anyhow::Result<()> {
        self.launch_process(row, false, None).await
    }
    pub(crate) async fn launch_snapshot(
        &self,
        row: &firemage_orm::vms::Model,
    ) -> anyhow::Result<()> {
        self.launch_process(row, true, None).await
    }
    pub(crate) async fn launch_snapshot_with_assets(
        &self,
        row: &firemage_orm::vms::Model,
        prepared_spec: &VmSpec,
    ) -> anyhow::Result<()> {
        self.launch_process(row, true, Some(prepared_spec)).await
    }
    async fn launch_process(
        &self,
        row: &firemage_orm::vms::Model,
        is_restore: bool,
        prepared_spec: Option<&VmSpec>,
    ) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        self.ensure_isolation_policy(&spec)?;
        if let Some(socket) = &spec.socket {
            crate::isolation::ensure_allowed_path(
                socket,
                self.config
                    .external_socket_roots
                    .as_deref()
                    .unwrap_or_default(),
                true,
            )?;
            return Ok(());
        }
        if is_restore {
            self.ensure_prepared_assets(row, prepared_spec.unwrap_or(&spec))
                .await?;
        }
        let args = self.config.firecracker_args.as_deref().unwrap_or_default();
        anyhow::ensure!(
            !args.iter().any(|arg| matches!(
                arg.as_str(),
                "--api-sock" | "--no-api" | "--log-path"
            ) || arg.starts_with("--api-sock=")
                || arg.starts_with("--log-path=")),
            "managed Firecracker requires its assigned API socket and log path"
        );
        let socket = std::path::Path::new(&row.socket);
        tokio::fs::create_dir_all(socket.parent().context("socket parent")?).await?;
        if socket.exists() {
            anyhow::ensure!(
                Firecracker::new(socket)?.state().await.is_err(),
                "VM socket already has a live Firecracker process"
            );
            tokio::fs::remove_file(socket).await?;
        }
        tokio::fs::create_dir_all(self.directory(&row.id)).await?;
        let logs = crate::logs::ProcessLogs::open(&self.directory(&row.id))?;
        let mut command = if spec.security.mode == IsolationMode::Jailed {
            anyhow::ensure!(
                unsafe { libc::geteuid() } == 0,
                "jailed VMs require root host privileges"
            );
            self.jail_uid(&row.id)?;
            if !is_restore {
                self.materialize_assets(row, &spec).await?;
            }
            self.jailed_command(row, &spec).await?
        } else {
            let mut command = tokio::process::Command::new(
                self.config
                    .firecracker
                    .as_deref()
                    .unwrap_or(std::path::Path::new("firecracker")),
            );
            command
                .args(self.config.firecracker_args.as_deref().unwrap_or_default())
                .args(["--api-sock", &row.socket])
                .arg("--log-path")
                .arg(self.directory(&row.id).join("firecracker.log"));
            command
        };
        if spec.security.mode != IsolationMode::Jailed {
            self.prepare_shell_directory(row, &spec).await?;
        }
        let child = command
            .env_clear()
            .stdin(if spec.terminal {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(logs.serial)
            .stderr(logs.stderr)
            .spawn()?;
        let pid = child.id().map(|id| id as i32);
        self.children.lock().await.insert(row.id.clone(), child);
        let process_id = pid.context("missing Firecracker process id")?;
        firemage_queries::set_process(
            &self.db,
            &row.id,
            process_id,
            crate::process::identity(process_id)?,
        )
        .await?;
        firemage_queries::set_vm_state(&self.db, row.clone(), "starting", None, pid).await?;
        let fc = Firecracker::new(socket)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if fc.state().await.is_ok() {
                if spec.security.mode == IsolationMode::Jailed {
                    self.secure_jail(row).await?;
                    self.prepare_shell_directory(row, &spec).await?;
                    self.stage_jail_resources(row, &spec).await?;
                }
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        anyhow::bail!("Firecracker socket did not become ready; inspect Firecracker logs")
    }
}
