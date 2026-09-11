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

pub async fn catalog_details(h: &Harness, alias: &str) -> Result<()> {
    let link = h.element(By::LinkText(alias)).await?;
    ensure!(link.tag_name().await? == "a", "egress alias is not a link");
    let current = h.driver.current_url().await?;
    ensure!(
        link.prop("href").await?.as_deref() == Some(current.as_str()),
        "selected egress alias lost its permalink"
    );
    h.element(By::Css("button[aria-label='Close egress details']"))
        .await?
        .click()
        .await?;
    h.absent(By::Css(".egress-catalog-drawer")).await?;
    let toggle = format!("button[aria-label='Toggle {alias} details']");
    h.element(By::Css(&toggle))
        .await?
        .send_keys(Key::Enter)
        .await?;
    h.element(By::Css(".egress-catalog-drawer")).await?;
    ensure!(
        h.element(By::Css(&toggle))
            .await?
            .attr("aria-expanded")
            .await?
            .as_deref()
            == Some("true"),
        "egress chevron lost expanded state"
    );
    h.element(By::XPath(format!("//table[contains(@class,'egress-catalog-table')]//tr[.//a[normalize-space()='{alias}']]/td[2]"))).await?.click().await?;
    h.absent(By::Css(".egress-catalog-drawer")).await?;
    h.element(By::LinkText(alias)).await?.click().await?;
    h.element(By::Css(".egress-catalog-drawer")).await?;
    Ok(())
}
