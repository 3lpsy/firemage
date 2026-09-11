use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::{components::SelectElement, prelude::*};

pub async fn assets(h: &Harness) -> Result<()> {
    h.login("admin").await?;
    h.navigate("Secrets").await?;
    h.button("+ Create secret").await?;
    h.fill("secret-name", "review-credentials").await?;
    h.fill("secret-value", "browser-attachment-private-value")
        .await?;
    h.modal_button("Save secret").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.navigate("Assets").await?;
    h.text("No assets yet").await?;
    h.absent(By::Id("asset-upload-section")).await?;
    h.button("+ Add asset").await?;
    h.element(By::Id("asset-upload-section")).await?;
    h.fill("asset-alias", "discarded-draft").await?;
    h.modal_button("Cancel").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.button("+ Add asset").await?;
    h.fill("asset-alias", "review-config").await?;
    let source = tempfile::NamedTempFile::new()?;
    std::fs::write(source.path(), b"review configuration")?;
    h.element(By::Id("asset-file"))
        .await?
        .send_keys(source.path().to_string_lossy().as_ref())
        .await?;
    h.screenshot("asset-upload-dialog").await?;
    h.modal_button("Add asset").await?;
    h.absent(By::Id("asset-upload-section")).await?;
    h.text("review-config").await?;
    anyhow::ensure!(
        h.driver.current_url().await?.fragment() == Some("assets"),
        "successful upload did not return to the asset list"
    );
    let before = h.api("/v1/assets").await?;
    let asset = before
        .as_array()
        .context("asset list")?
        .first()
        .context("uploaded asset")?;
    anyhow::ensure!(
        asset["alias"] == "review-config",
        "upload did not preserve alias"
    );
    let id = asset["id"].as_str().context("asset ID")?.to_owned();

    // A duplicate upload must retain the original file and show the server error.
    h.button("+ Add asset").await?;
    h.fill("asset-alias", "review-config").await?;
    h.element(By::Id("asset-file"))
        .await?
        .send_keys(source.path().to_string_lossy().as_ref())
        .await?;
    h.modal_button("Add asset").await?;
    h.element(By::Css("#asset-upload-section [role='alert']"))
        .await?;
    anyhow::ensure!(
        h.api("/v1/assets")
            .await?
            .as_array()
            .context("asset list")?
            .len()
            == 1,
        "duplicate alias created another asset"
    );
    h.button("Cancel").await?;
    h.element(By::Id("asset-search")).await?;
    h.button("+ Add asset").await?;
    h.radio("asset-source", "Remote URL").await?;
    h.fill("asset-alias", "remote-config").await?;
    h.fill("asset-import-url", "https://127.0.0.1/config.json")
        .await?;
    h.fill("asset-import-filename", "config.json").await?;
    h.modal_button("Add asset").await?;
    h.text("public destination").await?;
    h.screenshot("asset-remote-dialog").await?;
    h.modal_button("Cancel").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.navigate("Virtual machines").await?;
    h.button("+ Create VM").await?;
    h.fill("vm-name", "asset-review").await?;
    h.select_kernel("vmlinux").await?;
    h.radio("asset-source", "Local Disk").await?;
    h.fill("vm-rootfs", &h.asset("rootfs.ext4")).await?;
    h.button("+ Attach asset").await?;
    h.element(By::Id("vm-asset-0")).await?.click().await?;
    h.fill("vm-asset-search-0", "review-config").await?;
    h.element(By::Id("vm-asset-search-0"))
        .await?
        .send_keys(Key::Down + Key::Enter)
        .await?;
    h.element(By::Css("#vm-asset-0:focus")).await?;
    h.fill("asset-destination-0", "/etc/reviewer/config.json")
        .await?;
    h.element(By::Css(".attachment-row summary"))
        .await?
        .click()
        .await?;
    h.fill("asset-uid-0", "1000").await?;
    h.fill("asset-gid-0", "1000").await?;
    h.fill("asset-mode-0", "0600").await?;
    h.button("+ Attach asset").await?;
    SelectElement::new(&h.element(By::Id("attachment-source-1")).await?)
        .await?
        .select_by_value("secret")
        .await?;
    h.fill("vm-secret-file-1", "review-credentials").await?;
    h.fill("asset-destination-1", "/root/.config/reviewer/auth.json")
        .await?;
    h.element(By::XPath(
        "//input[@id='asset-mode-1']/ancestor::details[1]/summary",
    ))
    .await?
    .click()
    .await?;
    anyhow::ensure!(
        h.value("asset-mode-1").await? == "0600",
        "secret attachment did not default to private mode"
    );
    h.screenshot("vm-asset-create").await?;
    h.button("Create VM").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    let created = h.api("/v1/vms").await?;
    let vm = &created[0];
    anyhow::ensure!(vm["state"] == "defined", "creating a VM started it");
    let attachment = &vm["spec"]["attachments"][0];
    anyhow::ensure!(
        attachment["asset_id"] == id
            && attachment["destination"] == "/etc/reviewer/config.json"
            && attachment["uid"] == 1000
            && attachment["gid"] == 1000
            && attachment["mode"] == 384,
        "creation lost asset identity, destination or permissions"
    );
    anyhow::ensure!(
        vm["spec"]["secret_attachments"][0]["secret"] == "review-credentials"
            && vm["spec"]["secret_attachments"][0]["mode"] == 384
            && !vm.to_string().contains("browser-attachment-private-value"),
        "secret attachment was lost or exposed plaintext"
    );
    let vm_id = vm["id"].as_str().context("VM ID")?.to_owned();
    h.navigate("Assets").await?;
    h.row_button("review-config", "Edit alias").await?;
    h.fill("asset-edit-alias", "worker-config").await?;
    h.modal_button("Save alias").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.fill("asset-search", "WORKER").await?;
    h.text("worker-config").await?;
    anyhow::ensure!(
        h.api("/v1/assets").await?[0]["id"] == id,
        "alias edit changed asset identity"
    );
    let renamed = h.api("/v1/vms").await?;
    anyhow::ensure!(
        renamed[0]["spec"]["attachments"][0] == *attachment,
        "asset alias edit changed the VM attachment"
    );
    let delete = h
        .driver
        .find(By::XPath(
            "//tr[.//*[normalize-space(.)='worker-config']]//button[normalize-space(.)='Delete']",
        ))
        .await?;
    anyhow::ensure!(
        !delete.is_enabled().await?,
        "attached asset deletion was enabled"
    );
    h.screenshot("assets-library").await?;

    // Preserve fields managed outside the guided form when changing the destination.
    let mut seeded = vm["spec"].clone();
    seeded["environment"] = serde_json::json!({"REVIEW_MODE":"bugs"});
    seeded["files"] = serde_json::json!([{"path":"readme", "content":"review instructions", "encoding":"utf8", "destination":"/etc/reviewer/README", "uid":0, "gid":0, "mode":420}]);
    seeded["rootfs"] = serde_json::json!({"kind":"oci", "image":"registry.example.com/review:latest", "size_mib":4096});
    h.client
        .put(format!("{}/v1/vms/{vm_id}", h.url))
        .bearer_auth(&h.token)
        .json(&seeded)
        .send()
        .await?
        .error_for_status()?;
    h.navigate("Virtual machines").await?;
    h.vm_details("asset-review").await?;
    h.button("Attachments").await?;
    h.text("worker-config").await?;
    h.text("review-credentials").await?;
    h.text("/root/.config/reviewer/auth.json").await?;
    anyhow::ensure!(
        !h.driver
            .source()
            .await?
            .contains("browser-attachment-private-value"),
        "attachment table exposed secret contents"
    );
    h.screenshot("vm-attachments").await?;
    h.button("Edit attachments").await?;
    h.text("worker-config").await?;
    h.button("Full TOML").await?;
    let toml = h.value("vm-toml").await?;
    anyhow::ensure!(
        toml.contains("[[attachments]]") && toml.contains(&id),
        "TOML lost attachment identity"
    );
    h.button("Guided setup").await?;
    h.fill("asset-destination-0", "/opt/reviewer/config.json")
        .await?;
    h.screenshot("vm-asset-edit").await?;
    h.button("Save configuration").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    let edited = h.api("/v1/vms").await?;
    anyhow::ensure!(
        edited[0]["spec"]["environment"] == seeded["environment"]
            && edited[0]["spec"]["files"] == seeded["files"]
            && edited[0]["spec"]["rootfs"] == seeded["rootfs"]
            && edited[0]["spec"]["secret_attachments"] == seeded["secret_attachments"],
        "guided asset editing discarded environment, inline files or OCI disk size"
    );
    anyhow::ensure!(
        edited[0]["spec"]["attachments"][0]["asset_id"] == id
            && edited[0]["spec"]["attachments"][0]["destination"] == "/opt/reviewer/config.json"
            && edited[0]["spec"]["attachments"][0]["mode"] == 384,
        "guided edit lost asset identity, destination or permissions"
    );
    h.button("Attachments").await?;
    h.text("/opt/reviewer/config.json").await?;
    h.button("Overview").await?;
    h.button("Delete VM").await?;
    h.modal_button("Delete VM").await?;
    h.text("No virtual machines yet").await?;
    h.navigate("Assets").await?;
    h.fill("asset-search", "missing-file").await?;
    h.text("No matching assets").await?;
    h.fill("asset-search", "worker").await?;
    h.row_button("worker-config", "Delete").await?;
    h.modal_button("Delete asset").await?;
    h.text("No matching assets").await?;
    anyhow::ensure!(
        h.api("/v1/assets")
            .await?
            .as_array()
            .context("asset list")?
            .is_empty(),
        "deleted asset remains in library"
    );
    Ok(())
}
