use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::{components::SelectElement, prelude::*};

pub async fn registry(h: &Harness) -> Result<()> {
    h.navigate("Secrets").await?;
    h.button("+ Create secret").await?;
    h.fill("secret-name", "registry-password").await?;
    h.fill("secret-value", "browser-registry-private-value")
        .await?;
    h.modal_button("Save secret").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.navigate("Virtual machines").await?;
    h.button("+ Create VM").await?;
    h.fill("vm-name", "private-image").await?;
    h.select_kernel("vmlinux").await?;
    h.radio("asset-source", "OCI Image").await?;
    h.fill("vm-rootfs", "registry.example.com/jobs/runner:review")
        .await?;
    h.element(By::Css(".vm-form-registry > summary"))
        .await?
        .click()
        .await?;
    h.radio("registry-auth", "Bearer token").await?;
    SelectElement::new(&h.element(By::Id("registry-token")).await?)
        .await?
        .select_by_value("registry-password")
        .await?;
    h.radio("registry-auth", "Username and password").await?;
    h.fill("registry-username", "robot-build").await?;
    SelectElement::new(&h.element(By::Id("registry-password")).await?)
        .await?
        .select_by_value("registry-password")
        .await?;
    h.fill("registry-realm", "https://auth.example.com/token")
        .await?;
    h.screenshot("oci-registry").await?;
    h.button("Create VM").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    let vms = h.api("/v1/vms").await?;
    let registry = &vms[0]["spec"]["rootfs"]["registry"];
    anyhow::ensure!(
        registry["auth"]["kind"] == "basic"
            && registry["auth"]["username"] == "robot-build"
            && registry["auth"]["password_secret"] == "registry-password"
            && registry["auth"].get("token_secret").is_none()
            && registry["token_realm"] == "https://auth.example.com/token"
            && !vms.to_string().contains("browser-registry-private-value"),
        "registry configuration lost references, retained inactive auth, or exposed a secret"
    );
    h.navigate("Virtual machines").await?;
    h.vm_details("private-image").await?;
    h.button("Configure").await?;
    h.button("Full TOML").await?;
    let mut spec: toml::Value = toml::from_str(&h.value("vm-toml").await?)?;
    spec.as_table_mut()
        .context("VM configuration must be a table")?
        .insert("memory_mib".into(), 768.into());
    h.fill("vm-toml", &toml::to_string_pretty(&spec)?).await?;
    h.button("Save configuration").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    let edited = h.api("/v1/vms").await?;
    anyhow::ensure!(
        edited[0]["spec"]["rootfs"]["registry"] == *registry,
        "editing the VM discarded registry access settings"
    );
    h.button("Delete VM").await?;
    h.modal_button("Delete VM").await?;
    h.text("No virtual machines yet").await?;
    Ok(())
}
