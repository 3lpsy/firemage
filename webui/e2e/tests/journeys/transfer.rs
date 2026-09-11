use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use serde_json::{Value, json};
use thirtyfour::prelude::*;

pub async fn transfer(h: &Harness) -> Result<()> {
    let original = setup(h).await?;
    let original_id = original["id"].as_str().context("original ID")?;
    h.login("admin").await?;
    h.driver
        .goto(format!("{}#vms/{original_id}", h.url))
        .await?;
    h.element(By::Css(".vm-detail"))
        .await?
        .find(By::XPath(".//button[normalize-space(.)='Configuration']"))
        .await?
        .click()
        .await?;
    let source = h
        .element(By::Css("pre.config-export"))
        .await?
        .text()
        .await?;
    anyhow::ensure!(
        source.contains("kernel_alias = \"portable-linux\"")
            && source.contains("portable-source")
            && source.contains("portable-secret")
            && !source.contains("never-export-this-value")
            && !source.contains(
                original["spec"]["attachments"][0]["asset_id"]
                    .as_str()
                    .unwrap()
            ),
        "export did not use portable references or exposed a secret"
    );
    let download = h.element(By::Css("a[download='vm-config.toml']")).await?;
    anyhow::ensure!(
        download
            .attr("href")
            .await?
            .unwrap_or_default()
            .starts_with("data:application/toml;charset=utf-8,"),
        "export download is missing"
    );
    h.screenshot("vm-config-export").await?;
    let exact = h.api(&format!("/v1/vms/{original_id}/config")).await?["toml"]
        .as_str()
        .unwrap()
        .to_owned();
    super::clipboard::copy(h, "Copy VM configuration", &exact).await?;

    h.navigate("Virtual machines").await?;
    h.button("Import").await?;
    h.fill("vm-import-toml", &source).await?;
    h.fill("vm-import-name", "portable-import").await?;
    h.modal_button("Validate references").await?;
    h.text("Resolved references").await?;
    h.text("Guest IP: 10.78.0.3").await?;
    h.screenshot("vm-config-import-preview").await?;
    h.modal_button("Import VM").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let imported = find(h, "portable-import").await?;
    check_copy(&original, &imported, "10.78.0.3")?;

    h.driver
        .goto(format!("{}#vms/{original_id}", h.url))
        .await?;
    h.button("Duplicate VM").await?;
    h.fill("vm-duplicate-name", "portable-copy").await?;
    h.screenshot("vm-duplicate").await?;
    h.modal_button("Duplicate VM").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    let duplicate = find(h, "portable-copy").await?;
    check_copy(&original, &duplicate, "10.78.0.4")?;
    anyhow::ensure!(
        duplicate["spec"]["network"]["mac"] != imported["spec"]["network"]["mac"],
        "copies reused a MAC"
    );
    anyhow::ensure!(
        h.api(&format!("/v1/vms/{original_id}")).await?["spec"] == original["spec"],
        "transfer changed the source VM"
    );
    Ok(())
}

fn check_copy(original: &Value, copied: &Value, address: &str) -> Result<()> {
    anyhow::ensure!(
        copied["id"] != original["id"] && copied["state"] == "defined",
        "copy reused identity or started the VM"
    );
    let mut expected = original["spec"].clone();
    expected["name"] = copied["spec"]["name"].clone();
    expected["network"]["address"] = json!(address);
    expected["network"]["mac"] = copied["spec"]["network"]["mac"].clone();
    anyhow::ensure!(
        copied["spec"]["network"]["mac"] != original["spec"]["network"]["mac"]
            && copied["spec"] == expected,
        "copy lost configuration or fresh network identity"
    );
    Ok(())
}

async fn find(h: &Harness, name: &str) -> Result<Value> {
    h.api("/v1/vms")
        .await?
        .as_array()
        .context("VM list")?
        .iter()
        .find(|vm| vm["spec"]["name"] == name)
        .cloned()
        .context("copied VM")
}

async fn setup(h: &Harness) -> Result<Value> {
    h.client
        .put(format!("{}/v1/kernels/vmlinux", h.url))
        .bearer_auth(&h.token)
        .json(&json!({"alias":"portable-linux"}))
        .send()
        .await?
        .error_for_status()?;
    h.client
        .put(format!("{}/v1/secrets/portable-secret", h.url))
        .bearer_auth(&h.token)
        .json(&json!({"value":"never-export-this-value"}))
        .send()
        .await?
        .error_for_status()?;
    h.client.post(format!("{}/v1/networks", h.url)).bearer_auth(&h.token)
        .json(&json!({"name":"portable-net","subnet":"10.78.0.0/24","gateway":"10.78.0.1","policy":{"mode":"firemage-only"}}))
        .send().await?.error_for_status()?;
    let asset: Value = h
        .client
        .post(format!(
            "{}/v1/assets?alias=portable-source&filename=source.tar",
            h.url
        ))
        .bearer_auth(&h.token)
        .body("portable source")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let network = h.api("/v1/networks/portable-net/suggestion").await?;
    Ok(h.client
        .post(format!("{}/v1/vms", h.url))
        .bearer_auth(&h.token)
        .json(&json!({
            "name":"portable-original", "kernel":{"kind":"kernel","name":"vmlinux"},
            "rootfs":{"kind":"oci","image":"alpine:3.22","size_mib":2048},
            "workload":{"mode":"one-shot","command":["echo","review"]},
            "network":network,"environment":{"MODEL_KEY":{"secret":"portable-secret"}},
            "attachments":[{"asset_id":asset["id"],"destination":"/workspace/source.tar"}],
            "secret_attachments":[{"secret":"portable-secret","destination":"/root/auth.json"}],
            "metadata":{"job":"portable-review"}
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}
