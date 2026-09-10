use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::{components::SelectElement, prelude::*};

pub async fn egress(h: &Harness) -> Result<()> {
    h.login("admin").await?;
    h.navigate("Secrets").await?;
    h.button("+ Create secret").await?;
    h.fill("secret-name", "cloud-token").await?;
    h.fill("secret-value", "browser-fixture-private-value")
        .await?;
    h.modal_button("Save secret").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("cloud-token").await?;
    anyhow::ensure!(
        !h.api("/v1/secrets")
            .await?
            .to_string()
            .contains("browser-fixture-private-value"),
        "secret metadata exposed a value"
    );
    h.row_button("cloud-token", "Replace value").await?;
    anyhow::ensure!(
        h.value("secret-value").await?.is_empty(),
        "replace form revealed a saved secret"
    );
    h.fill("secret-value", "browser-fixture-replaced-value")
        .await?;
    h.modal_button("Save secret").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.screenshot("secrets").await?;

    h.navigate("Networks").await?;
    h.button("+ Create network").await?;
    h.fill("network-name", "proxy-net").await?;
    h.radio("network-policy", "Firemage only").await?;
    h.modal_button("Save network").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    anyhow::ensure!(
        h.api("/v1/networks").await?[0]["policy"]["mode"] == "firemage-only",
        "network mode was not saved"
    );
    h.navigate("Virtual machines").await?;
    h.button("+ Create VM").await?;
    h.fill("vm-name", "proxy-runner").await?;
    h.fill("vm-kernel", "/fixture/kernel").await?;
    h.fill("vm-rootfs", "/fixture/root.ext4").await?;
    SelectElement::new(&h.element(By::Id("vm-network")).await?)
        .await?
        .select_by_value("proxy-net")
        .await?;
    h.fill("vm-address", "172.30.0.2").await?;
    h.modal_button("Create VM").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.button("proxy-runner").await?;
    h.button("Egress").await?;
    h.button("Configure egress").await?;
    h.radio("egress-http-enabled", "Enabled").await?;
    h.button("OpenAI preset").await?;
    h.element(By::Css(".egress-credentials summary"))
        .await?
        .click()
        .await?;
    SelectElement::new(&h.element(By::Id("rule-0-header-0-secret")).await?)
        .await?
        .select_by_value("cloud-token")
        .await?;
    h.radio("rule-0-signing", "HMAC SHA-256").await?;
    SelectElement::new(&h.element(By::Id("rule-0-signing-key-secret")).await?)
        .await?
        .select_by_value("cloud-token")
        .await?;
    h.button("+ Add TCP tunnel").await?;
    h.fill("egress-tunnel-0-name", "database").await?;
    h.fill("egress-tunnel-0-target_host", "db.example.com")
        .await?;
    h.radio("egress-upstream-mode", "HTTP proxy").await?;
    h.fill("egress-upstream-url", "https://proxy.example.com:3128")
        .await?;
    h.fill("egress-upstream-username-literal", "runner").await?;
    h.radio("egress-upstream-password-mode", "Sensitive secret")
        .await?;
    SelectElement::new(&h.element(By::Id("egress-upstream-password-secret")).await?)
        .await?
        .select_by_value("cloud-token")
        .await?;
    h.screenshot("egress-editor").await?;
    h.modal_button("Save egress").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("api.openai.com").await?;
    h.text("database").await?;
    h.screenshot("egress-overview").await?;
    let vms = h.api("/v1/vms").await?;
    let spec = &vms[0]["spec"];
    anyhow::ensure!(
        spec["egress"]["http"]["rules"][0]["headers"]["Authorization"]["secret"] == "cloud-token",
        "header secret reference was lost"
    );
    anyhow::ensure!(
        spec["egress"]["http"]["rules"][0]["signing"]["kind"] == "hmac_sha256",
        "HMAC signing was not saved"
    );
    anyhow::ensure!(
        spec["egress"]["upstream"]["password"]["secret"] == "cloud-token",
        "upstream secret was not saved"
    );
    let id = vms[0]["id"].as_str().context("VM id")?;
    let status = h.api(&format!("/v1/vms/{id}/egress")).await?;
    anyhow::ensure!(
        status["enabled"] == true && status["active"] == false,
        "defined VM listener state is wrong"
    );
    super::boot_inputs::boot_inputs(h).await?;
    let spec = &h.api("/v1/vms").await?[0]["spec"];
    anyhow::ensure!(
        spec["egress"]["tunnels"][0]["target_host"] == "db.example.com",
        "boot/environment editing lost egress"
    );
    anyhow::ensure!(
        !spec.to_string().contains("browser-fixture-replaced-value"),
        "VM definition exposed resolved secret"
    );
    h.button("Egress").await?;
    h.button("Configure egress").await?;
    h.radio("egress-upstream-mode", "Direct").await?;
    h.modal_button("Save egress").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let spec = &h.api("/v1/vms").await?[0]["spec"];
    anyhow::ensure!(
        spec["egress"]["inherit_upstream"] == false && spec["egress"]["upstream"].is_null(),
        "explicit direct route was not saved"
    );
    h.screenshot("direct-route").await?;
    h.button("Configure egress").await?;
    h.element(By::Css(".egress-credentials summary"))
        .await?
        .click()
        .await?;
    h.radio("rule-0-signing", "AWS SigV4").await?;
    h.fill("rule-0-signing-region", "us-east-1").await?;
    h.fill("rule-0-signing-service", "execute-api").await?;
    for name in ["access_key", "secret_key"] {
        SelectElement::new(
            &h.element(By::Id(format!("rule-0-signing-{name}-secret")))
                .await?,
        )
        .await?
        .select_by_value("cloud-token")
        .await?;
    }
    h.radio("egress-upstream-mode", "SOCKS5 proxy").await?;
    h.fill("egress-upstream-url", "socks5://proxy.example.com:1080")
        .await?;
    h.modal_button("Save egress").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let spec = &h.api("/v1/vms").await?[0]["spec"];
    anyhow::ensure!(
        spec["egress"]["http"]["rules"][0]["signing"]["kind"] == "aws_sigv4",
        "AWS signing was not saved"
    );
    anyhow::ensure!(
        spec["egress"]["upstream"]["url"] == "socks5://proxy.example.com:1080",
        "SOCKS5 route was not saved"
    );
    Ok(())
}
