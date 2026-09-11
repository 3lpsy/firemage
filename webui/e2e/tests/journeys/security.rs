use anyhow::Result;
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

pub async fn limits(h: &Harness) -> Result<()> {
    h.button("Isolation").await?;
    h.text("Jailed").await?;
    h.element(By::Css("button[aria-label='About VM security boundaries']"))
        .await?
        .click()
        .await?;
    h.text("Guest root does not grant host root").await?;
    h.element(By::Css("button[aria-label='Close dialog']"))
        .await?
        .click()
        .await?;
    h.button("Edit limits").await?;
    h.element(By::Css("button[aria-label='About Host CPU quota']"))
        .await?;
    h.fill("security-memory_overhead_mib", "384").await?;
    h.fill("security-pids_max", "96").await?;
    h.fill("security-cpu_percent", "150").await?;
    h.fill("security-file_size_mib", "2048").await?;
    h.fill("security-open_files", "192").await?;
    h.screenshot("security-limits").await?;
    h.modal_button("Save host limits").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("896 MiB").await?;
    let vms = h.api("/v1/vms").await?;
    anyhow::ensure!(
        vms[0]["spec"]["security"]
            == serde_json::json!({
                "mode":"jailed", "memory_overhead_mib":384, "pids_max":96,
                "cpu_percent":150, "file_size_mib":2048, "open_files":192
            }),
        "security limits did not survive the API round trip"
    );
    anyhow::ensure!(
        vms[0]["spec"]["network"].is_null()
            && vms[0]["spec"]["userdata"] == "hello from the browser",
        "security editor changed guest permissions or boot inputs"
    );
    h.button("Edit limits").await?;
    h.fill("security-cpu_percent", "").await?;
    h.fill("security-file_size_mib", "").await?;
    h.modal_button("Save host limits").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let vms = h.api("/v1/vms").await?;
    anyhow::ensure!(
        vms[0]["spec"]["security"]["cpu_percent"].is_null()
            && vms[0]["spec"]["security"]["file_size_mib"].is_null(),
        "cleared host limits must restore automatic values"
    );
    h.screenshot("security-overview").await?;
    Ok(())
}
