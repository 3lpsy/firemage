use anyhow::Context;
use tokio::{
    io::{AsyncReadExt, AsyncWrite},
    process::Command,
};

pub(super) async fn run(
    command: &mut Command,
    output: &mut (impl AsyncWrite + Unpin),
    max_bytes: u64,
) -> anyhow::Result<u64> {
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("starting isolated guest filesystem reader")?;
    let mut stdout = child
        .stdout
        .take()
        .context("missing reader stdout")?
        .take(max_bytes.checked_add(1).context("invalid file limit")?);
    let mut stderr = child
        .stderr
        .take()
        .context("missing reader stderr")?
        .take(8193);
    let result = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let mut diagnostic = Vec::new();
        let (count, _) = tokio::try_join!(
            async {
                let count = tokio::io::copy(&mut stdout, output).await?;
                anyhow::ensure!(
                    count <= max_bytes,
                    "guest filesystem output exceeds configured limit"
                );
                anyhow::Ok(count)
            },
            async {
                stderr.read_to_end(&mut diagnostic).await?;
                anyhow::ensure!(
                    diagnostic.len() <= 8192,
                    "guest filesystem diagnostics exceeded limit"
                );
                anyhow::Ok(())
            },
        )?;
        let diagnostic = String::from_utf8_lossy(&diagnostic);
        anyhow::ensure!(
            child.wait().await?.success()
                && diagnostic.lines().all(|line| line.starts_with("debugfs ")),
            "guest filesystem read failed: {diagnostic}"
        );
        anyhow::Ok(count)
    })
    .await;
    match result {
        Ok(Ok(count)) => Ok(count),
        result => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match result {
                Ok(Err(error)) => Err(error),
                Err(_) => anyhow::bail!("guest filesystem read timed out after 60 seconds"),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}
