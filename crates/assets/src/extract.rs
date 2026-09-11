use anyhow::Context;
use firemage_wire::ensure_guest_path;
use std::path::Path;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

// Read stopped guest disks in an isolated parser with no host filesystem or network access.
pub async fn extract_jailed(disk: &Path, guest_path: &str, output: &Path) -> anyhow::Result<()> {
    ensure_guest_path(guest_path)?;
    let mut sandbox = crate::extract_sandbox::command(disk, guest_path).await
        .context("cannot prepare isolated output extraction; install bubblewrap and e2fsprogs and enable user namespaces")?;
    extract_with(&mut sandbox.command, output).await
}

// Explicit trusted mode permits running the host parser directly.
pub async fn extract(disk: &Path, guest_path: &str, output: &Path) -> anyhow::Result<()> {
    ensure_guest_path(guest_path)?;
    let mut command = Command::new("debugfs");
    command.args([
        "-R",
        &format!("cat /{guest_path}"),
        disk.to_str().context("invalid disk path")?,
    ]);
    extract_with(&mut command, output).await
}

async fn extract_with(command: &mut Command, output: &Path) -> anyhow::Result<()> {
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("starting guest output extraction")?;
    let mut stdout = child
        .stdout
        .take()
        .context("missing debugfs stdout")?
        .take(64 * 1024 * 1024 + 1);
    let mut stderr = child
        .stderr
        .take()
        .context("missing debugfs stderr")?
        .take(8193);
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let mut bytes = Vec::new();
        let mut diagnostic = Vec::new();
        tokio::try_join!(
            async {
                stdout.read_to_end(&mut bytes).await?;
                anyhow::ensure!(
                    bytes.len() <= 64 * 1024 * 1024,
                    "output file exceeds 64 MiB"
                );
                anyhow::Ok(())
            },
            async {
                stderr.read_to_end(&mut diagnostic).await?;
                anyhow::ensure!(
                    diagnostic.len() <= 8192,
                    "guest extraction diagnostics exceeded limit"
                );
                anyhow::Ok(())
            }
        )?;
        let diagnostic = String::from_utf8_lossy(&diagnostic);
        let status = child.wait().await?;
        anyhow::ensure!(
            status.success() && diagnostic.lines().all(|line| line.starts_with("debugfs ")),
            "guest file extraction failed: {diagnostic}"
        );
        let mut file = tokio::fs::File::create(output).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;
        anyhow::Ok(())
    })
    .await;
    match result {
        Ok(Ok(())) => Ok(()),
        result => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match result {
                Ok(Err(error)) => Err(error),
                Err(_) => anyhow::bail!("guest output extraction timed out after 30 seconds"),
                Ok(Ok(())) => unreachable!(),
            }
        }
    }
}
