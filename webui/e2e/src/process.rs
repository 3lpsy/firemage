use anyhow::Result;
use std::os::unix::process::CommandExt;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
pub(crate) struct Processes(pub Vec<Child>);
impl Drop for Processes {
    fn drop(&mut self) {
        for child in self.0.iter_mut().rev() {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(crate) fn free_port() -> Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port())
}
pub(crate) fn logged(command: &mut Command, path: PathBuf) -> Result<Child> {
    let log = std::fs::File::create(path)?;
    // Dedicated groups include Chromium descendants without touching other browsers.
    Ok(command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?)
}
pub(crate) fn browser_path() -> Option<String> {
    [
        "/usr/bin/chromium-browser",
        "/usr/bin/chromium",
        "/usr/bin/google-chrome",
    ]
    .into_iter()
    .find(|path| std::path::Path::new(path).is_file())
    .map(str::to_owned)
}
pub(crate) async fn ready(client: &reqwest::Client, url: &str, child: &mut Child) -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        anyhow::ensure!(
            child.try_wait()?.is_none(),
            "fixture process exited before readiness; inspect server/driver log"
        );
        if client
            .get(url)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "fixture readiness timed out: {url}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
