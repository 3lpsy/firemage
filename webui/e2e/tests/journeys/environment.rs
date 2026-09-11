use anyhow::{Context, Result, ensure};
use firemage_webui_e2e::Harness;
use serde_json::json;
use thirtyfour::prelude::*;

pub async fn variables(h: &Harness) -> Result<()> {
    h.button("Environment").await?;
    h.button("Add Variable").await?;
    h.fill("environment-0-name", "REGION").await?;
    h.fill("environment-0-literal", "initial-region").await?;
    h.button("+ Add variable").await?;
    h.fill("environment-1-name", "SERVICE_TOKEN").await?;
    h.radio("environment-1-mode", "Sensitive secret").await?;
    h.select_value("environment-1-secret", "cloud-token")
        .await?;
    h.button("+ Add variable").await?;
    h.fill("environment-2-name", "TEMPORARY").await?;
    h.fill("environment-2-literal", "remove-me").await?;
    h.screenshot("environment-editor").await?;
    h.modal_button("Add variables").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.element(By::Css(
        ".environment-table tr[data-environment-name='SERVICE_TOKEN']",
    ))
    .await?;
    h.text("Secret: cloud-token").await?;

    action(h, "Edit", "REGION").await?;
    ensure!(
        h.value("environment-0-name").await? == "REGION",
        "edit opened the wrong variable"
    );
    ensure!(
        h.driver
            .find_all(By::Id("environment-1-name"))
            .await?
            .is_empty(),
        "single-variable edit included another variable"
    );
    h.fill("environment-0-literal", "test-region").await?;
    let vms = h.api("/v1/vms").await?;
    let id = vms[0]["id"].as_str().context("VM id")?;
    let path = format!("/v1/vms/{id}");
    let mut latest = h.api(&path).await?["spec"].clone();
    latest["environment"]["CONCURRENT"] = json!("preserve-me");
    h.client
        .put(format!("{}{path}", h.url))
        .bearer_auth(&h.token)
        .json(&latest)
        .send()
        .await?
        .error_for_status()?;
    h.modal_button("Save variable").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("test-region").await?;
    let saved = h.api(&path).await?;
    ensure!(
        saved["spec"]["environment"]["CONCURRENT"] == "preserve-me",
        "editing overwrote a variable added while the modal was open"
    );
    ensure!(
        saved["spec"]["environment"]["SERVICE_TOKEN"]["secret"] == "cloud-token",
        "editing changed the secret reference"
    );
    ensure!(
        saved["spec"]["egress_policy"] == latest["egress_policy"],
        "editing changed unrelated VM configuration"
    );

    h.button("Add Variable").await?;
    h.fill("environment-0-name", "REGION").await?;
    h.fill("environment-0-literal", "must-not-overwrite")
        .await?;
    h.modal_button("Add variables").await?;
    h.text("REGION already exists. Use its Edit action to change it.")
        .await?;
    h.modal_button("Cancel").await?;
    h.absent(By::Css("[role='dialog']")).await?;

    action(h, "Delete", "TEMPORARY").await?;
    h.modal_button("Cancel").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    ensure!(
        h.api(&path).await?["spec"]["environment"]["TEMPORARY"] == "remove-me",
        "cancel removed a variable"
    );
    action(h, "Delete", "TEMPORARY").await?;
    h.modal_button("Delete variable").await?;
    h.absent(By::Css(
        ".environment-table tr[data-environment-name='TEMPORARY']",
    ))
    .await?;
    let saved = h.api(&path).await?;
    ensure!(
        saved["spec"]["environment"]
            == json!({"REGION":"test-region","SERVICE_TOKEN":{"secret":"cloud-token"},"CONCURRENT":"preserve-me"}),
        "delete changed another variable"
    );
    ensure!(
        !h.driver
            .source()
            .await?
            .contains("browser-fixture-replaced-value"),
        "environment UI revealed a secret value"
    );
    h.screenshot("environment-table").await?;
    Ok(())
}

async fn action(h: &Harness, action: &str, name: &str) -> Result<()> {
    h.element(By::Css(format!(
        "button[aria-label='{action} environment variable {name}']"
    )))
    .await?
    .click()
    .await?;
    Ok(())
}
