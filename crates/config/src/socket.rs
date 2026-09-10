use crate::Server;

impl Server {
    pub fn unix_socket_mode(&self) -> anyhow::Result<u32> {
        match self.unix_socket_mode.as_deref().unwrap_or("0600") {
            "0600" | "600" => Ok(0o600),
            "0660" | "660" => Ok(0o660),
            _ => anyhow::bail!("unix_socket_mode must be an octal string: 0600 or 0660"),
        }
    }

    pub(crate) fn validate_unix_socket(&self) -> anyhow::Result<()> {
        let mode = self.unix_socket_mode()?;
        anyhow::ensure!(
            self.unix_socket.is_some()
                || (self.unix_socket_mode.is_none() && self.unix_socket_gid.is_none()),
            "Unix socket permissions require unix_socket"
        );
        anyhow::ensure!(
            mode != 0o660 || self.unix_socket_gid.is_some(),
            "unix_socket_mode 0660 requires an explicit unix_socket_gid"
        );
        anyhow::ensure!(
            self.unix_socket_gid != Some(u32::MAX),
            "invalid Unix socket group ID"
        );
        Ok(())
    }
}
