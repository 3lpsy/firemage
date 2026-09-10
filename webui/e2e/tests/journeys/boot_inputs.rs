use anyhow::Result;
use firemage_webui_e2e::Harness;
use std::io::Write;
use thirtyfour::{components::SelectElement, prelude::*};

pub async fn boot_inputs(h: &Harness) -> Result<()> {
    h.button("Environment").await?;
    h.button("Configure environment").await?;
    h.button("+ Add variable").await?;
    h.fill("environment-0-name", "REGION").await?;
    h.fill("environment-0-literal", "test-region").await?;
    h.button("+ Add variable").await?;
    h.fill("environment-1-name", "SERVICE_TOKEN").await?;
    h.radio("environment-1-mode", "Sensitive secret").await?;
    SelectElement::new(&h.element(By::Id("environment-1-secret")).await?)
        .await?
        .select_by_value("cloud-token")
        .await?;
    h.screenshot("environment-editor").await?;
    h.modal_button("Save environment").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("Secret: cloud-token").await?;
    h.button("Boot").await?;
    h.button("Configure boot inputs").await?;
    h.button("+ Add boot file").await?;
    h.fill("boot-0-path", "runner-config").await?;
    h.fill("boot-0-destination", "/etc/runner/config.json")
        .await?;
    h.fill("boot-0-uid", "1000").await?;
    h.fill("boot-0-gid", "1000").await?;
    h.fill("boot-0-_mode", "0600").await?;
    h.fill("boot-0-content", "{\"enabled\":true}").await?;
    let mut upload = tempfile::NamedTempFile::new()?;
    upload.write_all(&[0, 255, 1, 254])?;
    h.button("+ Add boot file").await?;
    h.element(By::Id("boot-1-upload"))
        .await?
        .send_keys(upload.path().to_string_lossy().as_ref())
        .await?;
    h.fill("boot-1-path", "binary.dat").await?;
    h.fill("boot-1-destination", "/var/lib/runner/binary.dat")
        .await?;
    h.fill(
        "boot-userdata",
        "#!/bin/sh\nprintf '%s' \"$REGION\" > /tmp/region\n",
    )
    .await?;
    h.screenshot("boot-editor").await?;
    h.modal_button("Save boot inputs").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("/etc/runner/config.json").await?;
    let vms = h.api("/v1/vms").await?;
    let spec = &vms[0]["spec"];
    anyhow::ensure!(
        spec["files"][0]["mode"] == 384 && spec["files"][0]["uid"] == 1000,
        "boot owner and octal mode not preserved"
    );
    anyhow::ensure!(
        spec["environment"]["SERVICE_TOKEN"]["secret"] == "cloud-token"
            && spec["environment"]["REGION"] == "test-region",
        "environment sources not preserved"
    );
    anyhow::ensure!(
        spec["userdata"]
            .as_str()
            .unwrap_or_default()
            .contains("$REGION"),
        "userdata was not saved"
    );
    anyhow::ensure!(
        spec["files"][1]["content"] == "AP8B/g==" && spec["files"][1]["encoding"] == "base64",
        "binary upload did not preserve bytes"
    );
    Ok(())
}
