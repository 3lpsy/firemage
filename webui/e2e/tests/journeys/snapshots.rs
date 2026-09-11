use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use serde_json::json;
use thirtyfour::prelude::*;

pub async fn snapshots(h: &Harness) -> Result<()> {
    h.login("admin").await?;
    h.navigate("Snapshots").await?;
    h.text("No snapshots yet").await?;
    h.button("Save VM snapshot").await?;
    h.text("Pause a jailed VM before saving a snapshot.")
        .await?;
    anyhow::ensure!(
        !h.driver
            .find(By::Css("[role='dialog'] button[type='submit']"))
            .await?
            .is_enabled()
            .await?,
        "capture enabled without paused VM"
    );
    h.modal_button("Cancel").await?;

    let vm: serde_json::Value = h
        .client
        .post(format!("{}/v1/vms", h.url))
        .bearer_auth(&h.token)
        .json(&json!({"name":"snapshot-target", "memory_mib":64}))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    super::guest_files::install(h, vm["id"].as_str().context("VM ID")?).await?;
    h.navigate("Virtual machines").await?;
    h.vm_details("snapshot-target").await?;
    h.button("Files").await?;
    h.text("Stop the VM to browse and download files from its root disk.")
        .await?;
    h.absent(By::Css(".guest-files")).await?;
    super::guest_files::navigate(h).await?;
    h.client
        .post(format!("{}/v1/vms", h.url))
        .bearer_auth(&h.token)
        .json(&json!({"name":"snapshot-target", "memory_mib":64}))
        .send()
        .await?
        .error_for_status()?;
    let rows = h.api("/v1/vms").await?;
    let targets: Vec<_> = rows
        .as_array()
        .context("VM list")?
        .iter()
        .filter(|vm| vm["spec"]["name"] == "snapshot-target")
        .cloned()
        .collect();
    anyhow::ensure!(targets.len() == 2, "duplicate-name fixture missing");
    let target = &targets[1];
    let target_id = target["id"].as_str().context("target VM ID")?;
    let target_choice = format!("snapshot-target ({target_id})");
    h.navigate("Snapshots").await?;
    super::snapshot_targets::save(h, &targets, &target_choice, target_id).await?;

    let fixture = tempfile::tempdir()?;
    let archive = bundle(fixture.path())?;
    h.button("Upload snapshot").await?;
    h.fill("snapshot-upload-alias", "imported-checkpoint")
        .await?;
    h.element(By::Id("snapshot-upload-file"))
        .await?
        .send_keys(archive.to_string_lossy().as_ref())
        .await?;
    let trust = h
        .element(By::Css("[role='dialog'] input[type='checkbox']"))
        .await?;
    super::switches::keyboard_toggle(&trust).await?;
    h.screenshot("upload-trust-switch").await?;
    super::switches::keyboard_toggle(&trust).await?;
    h.modal_button("Upload snapshot").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("imported-checkpoint").await?;
    h.text("original-vm").await?;
    h.element(By::Css(".snapshot-drawer")).await?;
    h.text("Restore requirements").await?;
    h.text("snapshot-net").await?;
    h.text("192.0.2.0/24").await?;
    h.text("192.0.2.1").await?;
    h.text("192.0.2.2").await?;
    h.text("02:00:00:00:00:02").await?;
    anyhow::ensure!(
        h.element(By::Css(".snapshot-requirements"))
            .await?
            .text()
            .await?
            .contains("Read-only"),
        "snapshot requirements omitted read-only drive mode"
    );
    let rows = h.api("/v1/snapshots").await?;
    let snapshot = rows
        .as_array()
        .context("snapshot list")?
        .first()
        .context("uploaded snapshot")?;
    anyhow::ensure!(
        snapshot["trusted"] == false && snapshot["source_vm_id"].is_null(),
        "upload must not silently trust or bind an informational source name"
    );
    let id = snapshot["id"].as_str().context("snapshot ID")?;
    let picker = h.element(By::Css(".snapshot-restore input[list]")).await?;
    picker.send_keys("snapshot-target").await?;
    let restore = h
        .driver
        .find(By::Css(".snapshot-restore button.primary"))
        .await?;
    anyhow::ensure!(
        !restore.is_enabled().await?,
        "untrusted snapshot restore enabled before acknowledgement"
    );
    let trust = h
        .element(By::Css(".snapshot-restore input[type='checkbox']"))
        .await?;
    super::switches::keyboard_toggle(&trust).await?;
    anyhow::ensure!(
        !restore.is_enabled().await?,
        "restore accepted an ambiguous VM name"
    );
    picker.clear().await?;
    picker.send_keys(&target_choice).await?;
    h.element(By::Css(".snapshot-restore button.primary"))
        .await?
        .click()
        .await?;
    let confirmation = h.element(By::Css("[role='dialog']")).await?.text().await?;
    anyhow::ensure!(
        confirmation.contains(&target_choice)
            && confirmation.contains("Existing disk contents will be lost"),
        "restore confirmation omitted its target or destructive effect"
    );
    h.modal_button("Cancel").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let after_cancel = h.api("/v1/snapshots").await?;
    anyhow::ensure!(
        after_cancel[0]["trusted"] == false,
        "cancelling restore granted snapshot trust"
    );
    let target_after_cancel = h.api(&format!("/v1/vms/{target_id}")).await?;
    anyhow::ensure!(
        target_after_cancel["state"] == target["state"]
            && target_after_cancel["spec"] == target["spec"],
        "cancelling restore changed the target VM"
    );
    h.screenshot("snapshot-catalog").await?;

    let download = h.element(By::Css(".snapshot-drawer a[download]")).await?;
    anyhow::ensure!(
        download
            .attr("href")
            .await?
            .context("download URL")?
            .ends_with(&format!("/v1/snapshots/{id}/download")),
        "snapshot download does not use its catalog ID"
    );
    let bytes = h
        .client
        .get(format!("{}/v1/snapshots/{id}/download", h.url))
        .bearer_auth(&h.token)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    anyhow::ensure!(
        bytes.as_ref() == std::fs::read(&archive)?,
        "downloaded snapshot differs from uploaded bundle"
    );
    super::snapshot_targets::restore(h, target_id, id).await?;
    super::snapshot_targets::inventory(
        h,
        target_id,
        targets[0]["id"].as_str().context("other VM ID")?,
        id,
    )
    .await?;
    h.fill("snapshot-search", "nothing-matches").await?;
    h.absent(By::Css(".snapshot-table tbody tr")).await?;
    h.fill("snapshot-search", "original-vm").await?;
    h.element(By::Css(".snapshot-table tbody tr")).await?;
    h.button("Delete").await?;
    h.modal_button("Delete snapshot").await?;
    h.text("No snapshots yet").await?;
    anyhow::ensure!(
        h.api("/v1/snapshots")
            .await?
            .as_array()
            .context("snapshot list")?
            .is_empty(),
        "deleted snapshot remains in catalog"
    );
    Ok(())
}

// This exercises bundle transfer only; the placeholder state is never restored.
fn bundle(directory: &std::path::Path) -> Result<std::path::PathBuf> {
    std::fs::write(directory.join("state.bin"), b"browser-state-placeholder")?;
    std::fs::write(directory.join("kernel"), b"browser-kernel-placeholder")?;
    std::fs::write(directory.join("rootfs.ext4"), b"browser-disk-placeholder")?;
    std::fs::write(directory.join("initrd"), b"browser-initrd-placeholder")?;
    std::fs::write(
        directory.join("drive-data.img"),
        b"browser-data-placeholder",
    )?;
    firemage_snapshots::create_private(&directory.join("memory.bin"))?.set_len(64 * 1024 * 1024)?;
    let manifest = serde_json::from_value(json!({
        "version":1,"source_vm_name":"original-vm","architecture":"x86_64","firecracker_version":"1.16.1",
        "spec":{"name":"original-vm","memory_mib":64,
            "network":{"network":"snapshot-net","address":"192.0.2.2","mac":"02:00:00:00:00:02"},
            "initrd":{"kind":"local","path":"/images/initrd"},
            "drives":[{"id":"data","asset":{"kind":"local","path":"/images/data.ext4"},"read_only":true}]},
        "network":{"name":"snapshot-net","subnet":"192.0.2.0/24","gateway":"192.0.2.1","policy":{"mode":"isolated"}},"gateway_mac":"02:00:00:00:00:01",
        "files":{"state.bin":{"size_bytes":0,"sha256":""},"memory.bin":{"size_bytes":0,"sha256":""},"kernel":{"size_bytes":0,"sha256":""},"rootfs.ext4":{"size_bytes":0,"sha256":""},"initrd":{"size_bytes":0,"sha256":""},"drive-data.img":{"size_bytes":0,"sha256":""}}
    }))?;
    let archive = directory.join("browser.fmsnap");
    firemage_snapshots::pack(directory, manifest, &archive, 128 * 1024 * 1024)?;
    Ok(archive)
}
