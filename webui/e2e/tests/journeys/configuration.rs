use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

pub async fn configuration(h: &Harness) -> Result<()> {
    h.login("admin").await?;
    h.navigate("Host").await?;
    h.text("Runtime").await?;
    h.screenshot("host").await?;
    h.navigate("Configuration").await?;
    h.element(By::Id("server-toml")).await?;
    let original = h.value("server-toml").await?;
    h.fill("server-toml", "[server]\nsession_ttl = 1").await?;
    h.button("Validate & review").await?;
    h.text("session TTL must be 60-2592000 seconds").await?;
    anyhow::ensure!(
        std::fs::read_to_string(&h.config_path)?.contains("session_ttl = 600"),
        "invalid configuration changed the file"
    );
    h.fill("server-toml", &original).await?;
    h.button("Validate & review").await?;
    h.element(By::Css(
        "[role='dialog'][aria-label='Review configuration']",
    ))
    .await?;
    let external = format!(
        "{}\n# changed by another administrator\n",
        std::fs::read_to_string(&h.config_path)?
    );
    std::fs::write(&h.config_path, &external)?;
    h.modal_button("Save configuration").await?;
    h.text("configuration changed; reload before saving")
        .await?;
    anyhow::ensure!(
        std::fs::read_to_string(&h.config_path)? == external,
        "stale browser edit overwrote the other writer"
    );
    h.screenshot("revision-conflict").await?;
    h.driver.refresh().await?;
    h.element(By::Css(".app-shell")).await?;
    h.navigate("Configuration").await?;
    let mut document: toml::Value = toml::from_str(&h.value("server-toml").await?)?;
    let server = document
        .get_mut("server")
        .and_then(toml::Value::as_table_mut)
        .context("configuration editor has no server table")?;
    server.insert("session_ttl".into(), 900.into());
    server.insert(
        "firecracker_args".into(),
        toml::Value::Array(vec!["--level".into(), "Info".into()]),
    );
    h.fill("server-toml", &toml::to_string_pretty(&document)?)
        .await?;
    h.button("Validate & review").await?;
    h.text("firecracker_args is host-managed").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    document["server"]
        .as_table_mut()
        .context("server table")?
        .remove("firecracker_args");
    h.fill("server-toml", &toml::to_string_pretty(&document)?)
        .await?;
    h.button("Validate & review").await?;
    h.modal_button("Save configuration").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("Configuration saved.").await?;
    let view = h.api("/v1/config").await?;
    anyhow::ensure!(
        view["effective"]["session_ttl"] == 900,
        "saved session TTL was not applied live"
    );
    anyhow::ensure!(
        view["effective"]["firecracker_args"] == serde_json::json!([]),
        "host-only config changed through the API"
    );
    anyhow::ensure!(
        view["host_only"]
            .as_array()
            .is_some_and(|fields| fields.iter().any(|field| field == "firecracker_args")),
        "configuration UI must report host-only fields"
    );
    h.button("Effective settings").await?;
    h.text("900").await?;
    h.screenshot("effective-and-pending").await?;
    h.element(By::Css(
        "button[aria-label='About Configuration precedence']",
    ))
    .await?
    .click()
    .await?;
    h.text("Restart-required fields take effect").await?;
    h.screenshot("configuration-help").await?;
    h.element(By::Css("button[aria-label='Close dialog']"))
        .await?
        .click()
        .await?;
    h.navigate("Activity").await?;
    h.text("config.save").await?;
    Ok(())
}
