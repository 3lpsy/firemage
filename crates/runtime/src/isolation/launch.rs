use crate::Runtime;
use firemage_wire::VmSpec;
use std::path::Path;
impl Runtime {
    pub(crate) async fn jailed_command(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
    ) -> anyhow::Result<tokio::process::Command> {
        anyhow::ensure!(
            unsafe { libc::geteuid() } == 0,
            "jailed VMs require a root host provisioner with KVM, cgroup v2 and mount privileges"
        );
        let uid = self.jail_uid(&row.id)?;
        let jailer =
            super::executable(self.config.jailer.as_deref().unwrap_or(Path::new("jailer")))?;
        let firecracker = super::executable(
            self.config
                .firecracker
                .as_deref()
                .unwrap_or(Path::new("firecracker")),
        )?;
        let base = self.config.data_dir().join("jailer");
        tokio::fs::create_dir_all(&base).await?;
        let base = super::ensure_trusted_path(&base)?;
        let root = self.jail_root(row);
        anyhow::ensure!(
            firecracker
                .file_name()
                .is_some_and(|name| name == "firecracker"),
            "jailed Firecracker executable must be named firecracker"
        );
        if root.exists() {
            tokio::fs::remove_dir_all(&root).await?;
        }
        tokio::fs::create_dir_all(&root).await?;
        super::own(&root, 0, 0o700)?;
        self.cleanup_cgroup(row).await?;
        anyhow::ensure!(
            Path::new("/sys/fs/cgroup/cgroup.controllers").is_file(),
            "jailed VMs require cgroup v2"
        );
        let parent = self
            .config
            .jailer_cgroup_parent
            .as_deref()
            .unwrap_or("firemage");
        let socket = Path::new(&row.socket);
        if tokio::fs::symlink_metadata(socket).await.is_ok() {
            tokio::fs::remove_file(socket).await?;
        }
        std::os::unix::fs::symlink(std::fs::canonicalize(&root)?.join("api.sock"), socket)?;
        let file_limit = self.jail_file_limit(row, spec).await?;
        let mut command = tokio::process::Command::new(jailer);
        command
            .args(["--id", &row.id, "--exec-file"])
            .arg(firecracker)
            .args([
                "--uid",
                &uid.to_string(),
                "--gid",
                &uid.to_string(),
                "--chroot-base-dir",
            ])
            .arg(base)
            .args(["--cgroup-version", "2", "--parent-cgroup", parent])
            .args([
                "--cgroup",
                &format!(
                    "memory.max={}",
                    (u64::from(spec.memory_mib) + u64::from(spec.security.memory_overhead_mib))
                        * 1024
                        * 1024
                ),
            ])
            .args([
                "--cgroup",
                "memory.swap.max=0",
                "--cgroup",
                &format!("pids.max={}", spec.security.pids_max),
            ])
            .args([
                "--cgroup",
                &format!(
                    "cpu.max={} 100000",
                    spec.security
                        .cpu_percent
                        .unwrap_or(u32::from(spec.vcpus) * 100)
                        * 1000
                ),
            ])
            .args([
                "--resource-limit",
                &format!("no-file={}", spec.security.open_files),
                "--resource-limit",
                &format!("fsize={file_limit}"),
            ])
            .args(["--", "--api-sock", "/api.sock"]);
        Ok(command)
    }
    pub(crate) async fn secure_jail(&self, row: &firemage_orm::vms::Model) -> anyhow::Result<()> {
        let root = self.jail_root(row);
        super::own(&root, 0, 0o755)?;
        let resources = root.join("resources");
        tokio::fs::create_dir(&resources).await?;
        super::own(&resources, 0, 0o755)?;
        let snapshots = root.join("snapshots");
        tokio::fs::create_dir(&snapshots).await?;
        super::own(&snapshots, self.jail_uid(&row.id)?, 0o700)?;
        self.ensure_jail_process(row, false).await
    }
    pub(crate) async fn ensure_jail_process(
        &self,
        row: &firemage_orm::vms::Model,
        is_started: bool,
    ) -> anyhow::Result<()> {
        let pid = crate::process::socket_identity(&row.socket).await?.0;
        anyhow::ensure!(
            crate::process::is_same_file(
                Path::new(&format!("/proc/{pid}/root")),
                &self.jail_root(row)
            )?,
            "Firecracker is outside its assigned filesystem jail"
        );
        let membership = tokio::fs::read_to_string(format!("/proc/{pid}/cgroup")).await?;
        let expected = format!(
            "0::/{}/{}",
            self.config
                .jailer_cgroup_parent
                .as_deref()
                .unwrap_or("firemage"),
            row.id
        );
        anyhow::ensure!(
            membership.lines().any(|line| line == expected),
            "Firecracker did not enter its assigned cgroup"
        );
        let status = tokio::fs::read_to_string(format!("/proc/{pid}/status")).await?;
        let uid = self.jail_uid(&row.id)?;
        for field in ["Uid:", "Gid:"] {
            let values = status
                .lines()
                .find_map(|line| line.strip_prefix(field))
                .ok_or_else(|| anyhow::anyhow!("missing process identity"))?;
            anyhow::ensure!(
                values.split_whitespace().count() == 4
                    && values
                        .split_whitespace()
                        .all(|value| value.parse::<u32>().ok() == Some(uid)),
                "Firecracker did not drop to its assigned UID/GID"
            );
        }
        anyhow::ensure!(
            status
                .lines()
                .find_map(|line| line.strip_prefix("CapEff:"))
                .is_some_and(|value| value.trim() == "0000000000000000"),
            "Firecracker process retained effective capabilities"
        );
        // Firecracker installs the VMM/vCPU filters when building the guest, after the API filter.
        let mut tasks = tokio::fs::read_dir(format!("/proc/{pid}/task")).await?;
        let mut filtered = 0;
        while let Some(task) = tasks.next_entry().await? {
            let status = tokio::fs::read_to_string(task.path().join("status")).await?;
            let is_filtered =
                [("NoNewPrivs:", "1"), ("Seccomp:", "2")]
                    .iter()
                    .all(|(field, expected)| {
                        status
                            .lines()
                            .find_map(|line| line.strip_prefix(field))
                            .is_some_and(|value| value.trim() == *expected)
                    });
            anyhow::ensure!(
                !is_started || is_filtered,
                "running Firecracker thread lacks seccomp/no-new-privileges"
            );
            filtered += usize::from(is_filtered);
        }
        anyhow::ensure!(
            filtered > 0,
            "Firecracker API thread lacks seccomp/no-new-privileges"
        );
        Ok(())
    }
}
