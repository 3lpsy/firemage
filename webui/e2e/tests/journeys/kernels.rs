use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

pub async fn kernels(h: &Harness) -> Result<()> {
    h.login("admin").await?;
    h.navigate("Kernels").await?;
    h.text("vmlinux").await?;
    let refresh = h
        .element(By::Css("button[aria-label='Refresh kernels']"))
        .await?;
    anyhow::ensure!(
        refresh.text().await?.is_empty(),
        "kernel refresh is not icon-only"
    );
    let source = tempfile::NamedTempFile::new()?;
    std::fs::write(source.path(), b"uploaded browser kernel")?;
    h.button("+ Add kernel").await?;
    h.element(By::Css("button[aria-label='About Kernel alias']"))
        .await?
        .click()
        .await?;
    h.text("Renaming preserves existing VM selections.").await?;
    h.element(By::Css(
        "[role='dialog'][aria-label='Kernel alias'] button[aria-label='Close dialog']",
    ))
    .await?
    .click()
    .await?;
    h.fill("kernel-add-alias", "Browser kernel").await?;
    h.radio("kernel-source", "Remote URL").await?;
    let checksum = h.element(By::Id("kernel-sha")).await?;
    anyhow::ensure!(
        checksum.attr("required").await?.is_none()
            && checksum
                .attr("placeholder")
                .await?
                .is_some_and(|value| !value.is_empty()),
        "kernel checksum is not optional and explained"
    );
    h.fill("kernel-url", "https://127.0.0.1/kernel").await?;
    h.fill("kernel-name", "remote-fixture").await?;
    h.modal_button("Add kernel").await?;
    h.text("public destination").await?;
    h.fill("kernel-url", "https://example.com/kernel").await?;
    h.fill("kernel-sha", "invalid-checksum").await?;
    h.fill("kernel-name", "remote-fixture").await?;
    h.modal_button("Add kernel").await?;
    h.element(By::Css("[role='dialog'] [role='alert']")).await?;
    h.radio("kernel-source", "Upload file").await?;
    h.element(By::Id("kernel-upload"))
        .await?
        .send_keys(source.path().to_string_lossy().as_ref())
        .await?;
    h.fill("kernel-name", "uploaded-kernel").await?;
    h.modal_button("Add kernel").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("uploaded-kernel").await?;
    h.row_button("uploaded-kernel", "Edit alias").await?;
    h.element(By::Css("button[aria-label='About Kernel alias']"))
        .await?;
    h.fill("kernel-alias", "Recommended").await?;
    h.modal_button("Save alias").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.fill("kernel-search", "recommended").await?;
    h.text("uploaded-kernel").await?;
    h.screenshot("kernel-library").await?;

    h.navigate("Virtual machines").await?;
    h.button("+ Create VM").await?;
    h.fill("vm-name", "kernel-selection").await?;
    h.element(By::Id("vm-kernel")).await?.click().await?;
    h.fill("vm-kernel-search", "recommended").await?;
    h.screenshot("kernel-picker").await?;
    h.element(By::Id("vm-kernel-search"))
        .await?
        .send_keys(Key::Down + Key::Enter)
        .await?;
    h.element(By::Css("#vm-kernel:focus")).await?;
    h.radio("asset-source", "Local Disk").await?;
    h.fill("vm-rootfs", &h.asset("rootfs.ext4")).await?;
    h.button("Create VM").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    let vms = h.api("/v1/vms").await?;
    let vm = vms
        .as_array()
        .context("VM list")?
        .first()
        .context("created VM")?;
    anyhow::ensure!(
        vm["spec"]["kernel"]["kind"] == "kernel"
            && vm["spec"]["kernel"]["name"] == "uploaded-kernel",
        "picker did not persist catalog identity"
    );
    h.navigate("Kernels").await?;
    h.text("uploaded-kernel").await?;
    let delete = h
        .driver
        .find(By::XPath(
            "//tr[.//*[normalize-space(.)='uploaded-kernel']]//button[normalize-space(.)='Delete']",
        ))
        .await?;
    anyhow::ensure!(
        !delete.is_enabled().await?,
        "referenced kernel delete should be disabled"
    );
    let response = h
        .client
        .delete(format!("{}/v1/kernels/uploaded-kernel", h.url))
        .bearer_auth(&h.token)
        .send()
        .await?;
    anyhow::ensure!(
        !response.status().is_success(),
        "API deleted referenced kernel"
    );
    h.row_button("uploaded-kernel", "Edit alias").await?;
    h.fill("kernel-alias", "Renamed kernel").await?;
    h.modal_button("Save alias").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    anyhow::ensure!(
        h.api("/v1/vms").await?[0]["spec"]["kernel"]["name"] == "uploaded-kernel",
        "renaming alias changed VM identity"
    );
    h.navigate("Virtual machines").await?;
    h.vm_details("kernel-selection").await?;
    h.button("Delete VM").await?;
    h.modal_button("Delete VM").await?;
    h.text("No virtual machines yet").await?;
    h.navigate("Kernels").await?;
    h.row_button("uploaded-kernel", "Delete").await?;
    h.modal_button("Delete kernel").await?;
    h.absent(By::XPath("//td[normalize-space(.)='uploaded-kernel']"))
        .await?;
    let rows = h.api("/v1/kernels").await?;
    anyhow::ensure!(
        !rows
            .as_array()
            .context("kernel list")?
            .iter()
            .any(|row| row["name"] == "uploaded-kernel"),
        "deleted kernel remains in directory listing"
    );
    Ok(())
}
