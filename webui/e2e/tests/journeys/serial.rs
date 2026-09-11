use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

pub async fn serial(h: &Harness, vm_id: &str, screenshot: &str) -> Result<()> {
    let directory = h
        .config_path
        .parent()
        .context("fixture directory")?
        .join("data/vms")
        .join(vm_id);
    std::fs::create_dir_all(&directory)?;
    std::fs::write(
        directory.join("serial.log"),
        "guest-console-ready\r\n\x1b[31mred guest text\x1b[0m\r\n",
    )?;
    std::fs::write(directory.join("firecracker.log"), "host-api-diagnostic")?;
    std::fs::write(
        directory.join("firecracker-stderr.log"),
        "host-process-stderr",
    )?;
    std::fs::write(directory.join("console.log"), "legacy-combined-history")?;
    h.button("Serial").await?;
    h.text("guest-console-ready").await?;
    h.absent(By::XPath("//pre[contains(text(),'host-api-diagnostic')]"))
        .await?;
    h.button("View legacy combined output").await?;
    h.text("Legacy combined output").await?;
    h.text("legacy-combined-history").await?;
    h.button("Back to separate stream").await?;
    h.button("Enable terminal").await?;
    h.button("Terminal").await?;
    h.button("Connect terminal").await?;
    h.element(By::Css(".guest-terminal .xterm-screen")).await?;
    h.text("Waiting for the VM to run.").await?;
    h.element(By::Css(".guest-terminal .xterm-fg-1")).await?;
    h.screenshot(screenshot).await?;
    h.button("Disconnect").await?;
    h.absent(By::Css(".guest-terminal .xterm-screen")).await?;
    h.button("Disable terminal").await?;
    h.button("Firecracker").await?;
    h.text("host-api-diagnostic").await?;
    h.text("host-process-stderr").await?;
    h.absent(By::XPath("//pre[contains(text(),'guest-console-ready')]"))
        .await?;
    Ok(())
}
