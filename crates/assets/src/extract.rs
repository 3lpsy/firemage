use anyhow::Context;
use firemage_wire::ensure_guest_path;
use std::path::Path;

// Read a stopped guest disk without mounting guest-controlled filesystems on the host.
pub async fn extract(disk: &Path, guest_path: &str, output: &Path) -> anyhow::Result<()> {
    ensure_guest_path(guest_path)?;
    let output = output.to_str().context("invalid output path")?;
    anyhow::ensure!(
        !output.contains(['"', '\n', '\r', '\\']),
        "invalid output path"
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut child = tokio::process::Command::new("debugfs")
        .args([
            "-R",
            &format!("cat /{guest_path}"),
            disk.to_str().context("invalid disk path")?,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .context("missing debugfs stdout")?
        .take(64 * 1024 * 1024 + 1);
    let mut stderr = child
        .stderr
        .take()
        .context("missing debugfs stderr")?
        .take(8192);
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await?;
        anyhow::ensure!(
            bytes.len() <= 64 * 1024 * 1024,
            "output file exceeds 64 MiB"
        );
        let mut diagnostic = String::new();
        stderr.read_to_string(&mut diagnostic).await?;
        let status = child.wait().await?;
        anyhow::ensure!(
            status.success() && diagnostic.lines().all(|line| line.starts_with("debugfs ")),
            "guest file extraction failed: {diagnostic}"
        );
        let mut file = tokio::fs::File::create(output).await?;
        file.write_all(&bytes).await?;
        anyhow::Ok(())
    })
    .await?;
    if result.is_err() {
        let _ = child.kill().await;
    }
    result?;
    anyhow::ensure!(Path::new(output).is_file(), "guest file not found");
    Ok(())
}
