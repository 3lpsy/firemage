use crate::Runtime;
use firemage_wire::{IsolationMode, VmSpec};
impl Runtime {
    pub(crate) fn ensure_isolation_policy(&self, spec: &VmSpec) -> anyhow::Result<()> {
        spec.validate()?;
        if let Some(kernel) = &spec.kernel {
            self.kernel_name(kernel)?;
        }
        for asset in spec
            .rootfs
            .iter()
            .chain(spec.initrd.iter())
            .chain(spec.drives.iter().map(|drive| &drive.asset))
        {
            if let firemage_wire::Asset::Local { path } = asset {
                super::ensure_allowed_path(
                    path,
                    self.config.local_asset_roots.as_deref().unwrap_or_default(),
                    false,
                )?;
            }
        }
        if let Some(socket) = &spec.socket {
            super::ensure_allowed_path(
                socket,
                self.config
                    .external_socket_roots
                    .as_deref()
                    .unwrap_or_default(),
                false,
            )?;
        }
        match spec.security.mode {
            IsolationMode::Jailed => anyhow::ensure!(
                self.config
                    .firecracker_args
                    .as_ref()
                    .is_none_or(Vec::is_empty),
                "jailed VMs require built-in Firecracker seccomp and arguments; custom arguments require trusted mode"
            ),
            IsolationMode::Trusted => anyhow::ensure!(
                self.config.allow_trusted_vms == Some(true),
                "trusted VMs are disabled by host policy"
            ),
            IsolationMode::External => anyhow::ensure!(
                self.config.allow_external_vms == Some(true),
                "external VMs are disabled by host policy"
            ),
        }
        Ok(())
    }
    pub(crate) fn jail_root(&self, row: &firemage_orm::vms::Model) -> std::path::PathBuf {
        self.config
            .data_dir()
            .join("jailer/firecracker")
            .join(&row.id)
            .join("root")
    }
    pub(crate) async fn cleanup_cgroup(
        &self,
        row: &firemage_orm::vms::Model,
    ) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.security.mode != IsolationMode::Jailed {
            return Ok(());
        }
        let path = self.cgroup_path(&row.id);
        if path.exists() {
            anyhow::ensure!(
                tokio::fs::read_to_string(path.join("cgroup.procs"))
                    .await?
                    .trim()
                    .is_empty(),
                "VM cgroup still contains processes"
            );
            tokio::fs::remove_dir(path).await?;
        }
        Ok(())
    }
    pub(crate) fn cgroup_path(&self, id: &str) -> std::path::PathBuf {
        std::path::Path::new("/sys/fs/cgroup")
            .join(
                self.config
                    .jailer_cgroup_parent
                    .as_deref()
                    .unwrap_or("firemage"),
            )
            .join(id)
    }
}
