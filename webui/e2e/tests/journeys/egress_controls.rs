use anyhow::{Result, ensure};
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

pub async fn folded_http_switch(h: &Harness) -> Result<()> {
    let section = h.element(By::Id("policy-section-http")).await?;
    section.find(By::Css("summary")).await?.click().await?;
    let toggle = h.element(By::Id("egress-http-enabled")).await?;
    ensure!(toggle.attr("role").await?.as_deref() == Some("switch"));
    ensure!(toggle.is_selected().await?, "HTTP preset is not enabled");
    toggle.click().await?;
    ensure!(!toggle.is_selected().await?, "HTTP switch did not disable");
    ensure!(
        section.attr("open").await?.is_none(),
        "HTTP switch unfolded its section"
    );
    section.find(By::Css("summary")).await?.click().await?;
    ensure!(
        h.driver.find_all(By::Css(".egress-rule")).await?.is_empty(),
        "disabled HTTP proxy still shows rule fields"
    );
    toggle.click().await?;
    ensure!(
        section.attr("open").await?.is_some(),
        "HTTP switch folded its section"
    );
    let host = h.element(By::Id("egress-rule-0-host")).await?;
    ensure!(
        host.prop("value").await?.as_deref() == Some("api.openai.com"),
        "HTTP switch lost draft rules"
    );
    Ok(())
}
