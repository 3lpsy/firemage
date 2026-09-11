use crate::Runtime;
use anyhow::Context;
use firemage_wire::{Asset, BootFile, FileEncoding, IsolationMode, VmSpec};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) const SHELL_CID: u32 = 3;
pub(crate) const JAILED_SHELL_SOCKET: &str = "/shell/vsock.sock";

impl Runtime {
    pub(crate) async fn guest_binary_file(&self) -> anyhow::Result<BootFile> {
        use base64::Engine;
        let bytes = if let Some(path) = &self.config.firemage_guest_bin_path {
            let file = tokio::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)
                .await
                .context("cannot open firemage_guest_bin_path")?;
            let metadata = file.metadata().await?;
            anyhow::ensure!(
                metadata.is_file() && metadata.len() <= 32 * 1024 * 1024,
                "firemage_guest_bin_path must be a regular binary up to 32 MiB"
            );
            let mut bytes = Vec::new();
            file.take(32 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .await?;
            anyhow::ensure!(
                bytes.len() <= 32 * 1024 * 1024,
                "guest binary exceeds 32 MiB"
            );
            bytes
        } else {
            firemage_guest_bundle::bytes().to_vec()
        };
        firemage_guest_bundle::validate(&bytes)
            .map_err(anyhow::Error::msg)
            .context("guest helper must be a static Linux executable")?;
        Ok(BootFile {
            path: "firemage/firemage-guest".into(),
            content: base64::engine::general_purpose::STANDARD.encode(bytes),
            encoding: FileEncoding::Base64,
            destination: None,
            uid: 0,
            gid: 0,
            mode: 0o700,
        })
    }

    pub(crate) fn ensure_web_terminal_init(&self, id: &str, spec: &VmSpec) -> anyhow::Result<()> {
        if spec.web_terminal.is_some()
            && matches!(spec.rootfs, Some(Asset::Oci { .. }))
            && self.directory(id).join("rootfs.ext4").try_exists()?
        {
            anyhow::ensure!(
                std::fs::read(self.directory(id).join("oci-init-version"))
                    .is_ok_and(|version| version == b"1\n" || version == b"2\n"),
                "this prepared OCI disk has no supported managed init; a compatible guest init must run the input disk setup script"
            );
        }
        Ok(())
    }

    pub(crate) fn shell_socket(&self, row: &firemage_orm::vms::Model, spec: &VmSpec) -> PathBuf {
        if spec.security.mode == IsolationMode::Jailed {
            self.jail_root(row).join("shell/vsock.sock")
        } else {
            self.directory(&row.id).join("shell/vsock.sock")
        }
    }

    pub(crate) async fn prepare_shell_directory(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
    ) -> anyhow::Result<()> {
        if spec.web_terminal.is_none() {
            return Ok(());
        }
        let socket = self.shell_socket(row, spec);
        anyhow::ensure!(
            spec.security.mode == IsolationMode::Jailed || socket.as_os_str().len() < 108,
            "Web Terminal socket path exceeds Linux limit"
        );
        let parent = socket.parent().context("missing shell socket parent")?;
        if parent.exists() {
            tokio::fs::remove_dir_all(parent).await?;
        }
        tokio::fs::create_dir_all(parent).await?;
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).await?;
        if spec.security.mode == IsolationMode::Jailed {
            crate::isolation::own(parent, self.jail_uid(&row.id)?, 0o700)?;
        }
        Ok(())
    }

    pub(crate) fn shell_device_path(
        &self,
        row: &firemage_orm::vms::Model,
        spec: &VmSpec,
    ) -> PathBuf {
        if spec.security.mode == IsolationMode::Jailed {
            JAILED_SHELL_SOCKET.into()
        } else {
            self.shell_socket(row, spec)
        }
    }

    pub async fn ensure_web_shell(&self, owner: &str, id: &str) -> anyhow::Result<()> {
        let row = firemage_queries::vm(&self.db, owner, id).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        anyhow::ensure!(
            spec.web_terminal.is_some() && spec.socket.is_none(),
            "Web Terminal is disabled"
        );
        anyhow::ensure!(row.state == "running", "Web Shell requires a running VM");
        Ok(())
    }

    // Resolve the assigned socket under the VM lock; clients never supply host paths or commands.
    pub async fn connect_web_shell(
        &self,
        owner: &str,
        id: &str,
    ) -> anyhow::Result<(tokio::net::UnixStream, Vec<String>)> {
        let _guard = self.lock(id).await;
        self.ensure_web_shell(owner, id).await?;
        let row = firemage_queries::vm(&self.db, owner, id).await?;
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.security.mode == IsolationMode::Jailed {
            self.ensure_jail_process(&row, true).await?;
        }
        let command = spec
            .web_terminal
            .as_ref()
            .context("Web Terminal is disabled")?
            .command
            .clone();
        let socket = self.shell_socket(&row, &spec);
        let stream = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            use std::os::fd::AsRawFd;
            let parent = tokio::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
                .open(socket.parent().context("missing guest socket parent")?)
                .await?;
            let short_path = format!("/proc/self/fd/{}/vsock.sock", parent.as_raw_fd());
            let mut stream = tokio::net::UnixStream::connect(short_path).await?;
            drop(parent);
            let peer = stream
                .peer_cred()?
                .pid()
                .context("guest socket has no process identity")?;
            anyhow::ensure!(
                row.pid == Some(peer)
                    && row
                        .process_start
                        .as_deref()
                        .is_some_and(|start| crate::process::is_alive(peer, start)),
                "guest socket is outside the assigned VM process"
            );
            stream
                .write_all(format!("CONNECT {}\n", firemage_guest_protocol::PORT).as_bytes())
                .await?;
            let mut line = Vec::new();
            loop {
                let byte = stream.read_u8().await?;
                if byte == b'\n' {
                    break;
                }
                anyhow::ensure!(line.len() < 128, "invalid guest connection response");
                line.push(byte);
            }
            anyhow::ensure!(
                std::str::from_utf8(&line)
                    .ok()
                    .and_then(|line| line.strip_prefix("OK "))
                    .and_then(|port| port.parse::<u32>().ok())
                    .is_some_and(|port| port > 0),
                "guest shell connection rejected"
            );
            anyhow::Ok(stream)
        })
        .await
        .context("guest helper did not respond; verify guest vsock support and init setup")??;
        Ok((stream, command))
    }
}

#[cfg(test)]
#[path = "web_shell_tests.rs"]
mod tests;
