use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

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
    h.navigate("Virtual machines").await?;
    h.button("+ Create VM").await?;
    h.fill("vm-name", "proxy-runner").await?;
    h.select_kernel("kernel").await?;
    h.radio("asset-source", "Local Disk").await?;
    h.fill("vm-rootfs", &h.asset("root.ext4")).await?;
    h.fill("vm-memory", "").await?;
    h.button("Network").await?;
    h.select_network("proxy-net").await?;
    h.fill("vm-address", "172.30.0.2").await?;
    h.element(By::Css("button[aria-controls='vm-section-egress']"))
        .await?
        .click()
        .await?;
    h.element(By::Css("button[aria-label='Create Egress policy']"))
        .await?
        .click()
        .await?;
    h.fill("policy-alias", "shared-review").await?;
    h.element(By::Id("egress-http-enabled"))
        .await?
        .click()
        .await?;
    h.button("OpenAI preset").await?;
    super::egress_controls::folded_http_switch(h).await?;
    h.element(By::Css(".egress-credentials summary"))
        .await?
        .click()
        .await?;
    h.select_value("rule-0-header-0-secret", "cloud-token")
        .await?;
    h.radio("rule-0-signing", "HMAC SHA-256").await?;
    h.select_value("rule-0-signing-key-secret", "cloud-token")
        .await?;
    h.button("TCP tunnels").await?;
    h.button("+ Add TCP tunnel").await?;
    h.fill("egress-tunnel-0-name", "database").await?;
    h.fill("egress-tunnel-0-target_host", "db.example.com")
        .await?;
    h.element(By::Css("button[aria-label='Create Upstream proxy']"))
        .await?
        .click()
        .await?;
    h.fill("proxy-alias", "shared-exit").await?;
    h.fill("proxy-url", "https://proxy.example.com:3128")
        .await?;
    h.fill("proxy-username-literal", "runner").await?;
    h.select_value("proxy-password-secret", "cloud-token")
        .await?;
    h.modal_button("Create proxy").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let upstream = h.element(By::Id("policy-upstream")).await?;
    anyhow::ensure!(
        upstream
            .find(By::Css("option:checked"))
            .await?
            .text()
            .await?
            == "shared-exit",
        "newly created proxy remained unavailable in the policy selector"
    );
    anyhow::ensure!(
        h.value("rule-0-signing-key-secret").await? == "cloud-token",
        "creating a proxy lost signing credentials"
    );
    h.element(By::Css(".egress-editor-page h1"))
        .await?
        .scroll_into_view()
        .await?;
    h.screenshot("policy-page").await?;
    h.button("Create and select").await?;
    h.element(By::Id("vm-name")).await?;
    anyhow::ensure!(
        h.value("vm-name").await? == "proxy-runner" && h.value("vm-memory").await?.is_empty(),
        "creating a policy lost unfinished VM fields"
    );
    h.fill("vm-memory", "256").await?;
    h.button("Create VM").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    h.navigate("Virtual machines").await?;
    h.vm_details("proxy-runner").await?;
    vm_egress(h).await?;
    h.text("api.openai.com").await?;
    let vms = h.api("/v1/vms").await?;
    let id = vms[0]["id"].as_str().context("VM id")?;
    let policy_id = vms[0]["spec"]["egress_policy"]
        .as_str()
        .context("policy reference")?;
    let policy_path = format!("/v1/egress/policies/{policy_id}");
    let policy = h.api(&policy_path).await?;
    anyhow::ensure!(
        policy["policy"]["http"]["rules"][0]["headers"]["Authorization"]["secret"] == "cloud-token",
        "header secret reference was lost"
    );
    anyhow::ensure!(
        policy["policy"]["http"]["rules"][0]["signing"]["kind"] == "hmac_sha256",
        "HMAC signing was not saved"
    );
    let proxy_id = policy["upstream_proxy_id"]
        .as_str()
        .context("proxy reference")?;
    let proxy = h.api(&format!("/v1/egress/proxies/{proxy_id}")).await?;
    anyhow::ensure!(
        proxy["proxy"]["password"]["secret"] == "cloud-token",
        "upstream secret was not saved"
    );
    let status = h.api(&format!("/v1/vms/{id}/egress")).await?;
    anyhow::ensure!(
        status["enabled"] == true && status["active"] == false,
        "defined VM listener state is wrong"
    );
    super::boot_inputs::boot_inputs(h).await?;
    let spec = &h.api("/v1/vms").await?[0]["spec"];
    anyhow::ensure!(
        spec["egress_policy"] == policy_id,
        "boot/environment editing lost policy binding"
    );
    anyhow::ensure!(
        !spec.to_string().contains("browser-fixture-replaced-value"),
        "VM definition exposed resolved secret"
    );
    h.button("Duplicate VM").await?;
    h.fill("vm-duplicate-name", "proxy-runner-copy").await?;
    h.modal_button("Duplicate VM").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let copies = h.api("/v1/vms").await?;
    anyhow::ensure!(
        copies
            .as_array()
            .context("VM list")?
            .iter()
            .all(|vm| vm["spec"]["egress_policy"] == policy_id),
        "duplicate did not reuse the shared policy"
    );
    h.navigate("Egress").await?;
    h.element(By::LinkText("shared-review"))
        .await?
        .click()
        .await?;
    super::egress_controls::catalog_details(h, "shared-review").await?;
    h.text("Used by 2 VMs").await?;
    let disabled = h
        .driver
        .find(By::XPath("//button[normalize-space(.)='Delete policy']"))
        .await?;
    anyhow::ensure!(
        !disabled.is_enabled().await?,
        "policy used by a defined VM can be deleted"
    );
    h.text("proxy-runner").await?;
    h.button("Upstream proxies").await?;
    h.element(By::LinkText("shared-exit"))
        .await?
        .click()
        .await?;
    super::egress_controls::catalog_details(h, "shared-exit").await?;
    h.text("Used by 1 policies").await?;
    let disabled = h
        .driver
        .find(By::XPath("//button[normalize-space(.)='Delete proxy']"))
        .await?;
    anyhow::ensure!(
        !disabled.is_enabled().await?,
        "referenced proxy can be deleted"
    );
    h.button("Egress policies").await?;
    h.element(By::LinkText("shared-review"))
        .await?
        .click()
        .await?;
    h.button("Edit policy").await?;
    anyhow::ensure!(
        h.value("policy-upstream").await? == proxy_id,
        "existing upstream was not selected after loading"
    );
    h.select_value("policy-upstream", "").await?;
    h.element(By::Css(".egress-credentials summary"))
        .await?
        .click()
        .await?;
    h.radio("rule-0-signing", "AWS SigV4").await?;
    h.fill("rule-0-signing-region", "us-east-1").await?;
    h.fill("rule-0-signing-service", "execute-api").await?;
    for name in ["access_key", "secret_key"] {
        h.select_value(&format!("rule-0-signing-{name}-secret"), "cloud-token")
            .await?;
    }
    anyhow::ensure!(
        h.value("policy-upstream").await?.is_empty(),
        "direct route selection was lost after signing edits"
    );
    h.button("Save policy").await?;
    h.absent(By::Css(".egress-editor-page")).await?;
    let saved = h.api(&policy_path).await?;
    anyhow::ensure!(
        saved["policy"]["inherit_upstream"] == false && saved["upstream_proxy_id"].is_null(),
        "direct route was not saved"
    );
    anyhow::ensure!(
        saved["policy"]["http"]["rules"][0]["signing"]["kind"] == "aws_sigv4",
        "AWS signing was not saved"
    );
    h.button("Upstream proxies").await?;
    h.element(By::LinkText("shared-exit"))
        .await?
        .click()
        .await?;
    h.button("Edit proxy").await?;
    h.fill("proxy-url", "socks5://proxy.example.com:1080")
        .await?;
    h.modal_button("Save proxy").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let proxy = h.api(&format!("/v1/egress/proxies/{proxy_id}")).await?;
    anyhow::ensure!(
        proxy["proxy"]["url"] == "socks5://proxy.example.com:1080",
        "SOCKS5 proxy was not saved"
    );
    h.screenshot("upstream-catalog").await?;
    super::egress_loading::catalog_switching(h).await?;
    Ok(())
}

async fn vm_egress(h: &Harness) -> Result<()> {
    h.element(By::XPath(
        "//*[@id='vm-detail']//button[normalize-space(.)='Egress']",
    ))
    .await?
    .click()
    .await?;
    Ok(())
}
