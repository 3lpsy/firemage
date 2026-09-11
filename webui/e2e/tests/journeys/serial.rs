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
    set_terminal(h, vm_id, true).await?;
    h.button("Serial").await?;
    h.text("guest-console-ready").await?;
    h.absent(By::XPath("//pre[contains(text(),'host-api-diagnostic')]"))
        .await?;
    h.button("View legacy combined output").await?;
    h.text("Legacy combined output").await?;
    h.text("legacy-combined-history").await?;
    h.button("Back to separate stream").await?;
    super::serial_connection::connection(h, vm_id, screenshot).await?;
    set_terminal(h, vm_id, false).await?;
    h.button("Firecracker Logs").await?;
    h.text("host-api-diagnostic").await?;
    h.text("host-process-stderr").await?;
    h.absent(By::XPath("//pre[contains(text(),'guest-console-ready')]"))
        .await?;
    Ok(())
}

async fn set_terminal(h: &Harness, id: &str, enabled: bool) -> Result<()> {
    let path = format!("/v1/vms/{id}");
    let mut spec = h.api(&path).await?["spec"].clone();
    spec["terminal"] = enabled.into();
    h.client
        .put(format!("{}{}", h.url, path))
        .bearer_auth(&h.token)
        .json(&spec)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
