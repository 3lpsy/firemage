use anyhow::Result;
use firemage_webui_e2e::{Harness, PASSWORD};
use thirtyfour::prelude::*;

pub async fn oidc(h: &Harness) -> Result<()> {
    sign_in(h, "unlinked").await?;
    h.element(By::Id("login-username")).await?;
    anyhow::ensure!(
        h.driver.current_url().await?.query() == Some("login_error=oidc"),
        "unlinked OIDC identity was not rejected"
    );
    h.screenshot("unlinked-rejected").await?;
    sign_in(h, "federated").await?;
    h.element(By::Css(".app-shell")).await?;
    h.element(By::XPath(
        "//div[contains(@class,'account')]//strong[normalize-space(.)='federated']",
    ))
    .await?;
    h.absent(By::XPath("//nav//button[contains(.,'Users')]"))
        .await?;
    h.screenshot("federated-session").await?;
    h.driver.refresh().await?;
    h.element(By::Css(".app-shell")).await?;
    h.navigate("API tokens").await?;
    h.text("No API tokens").await?;
    let callback = h.oidc_callback()?;
    h.logout().await?;
    h.driver.goto(&callback).await?;
    h.element(By::Id("login-username")).await?;
    anyhow::ensure!(
        h.driver
            .current_url()
            .await?
            .query()
            .is_some_and(|query| query.contains("login_error=oidc")),
        "replayed OIDC callback was accepted"
    );
    h.screenshot("replay-rejected").await?;
    Ok(())
}
async fn sign_in(h: &Harness, username: &str) -> Result<()> {
    h.element(By::LinkText("Continue with OpenID Connect"))
        .await?
        .click()
        .await?;
    h.fill("provider-username", username).await?;
    h.fill("provider-password", PASSWORD).await?;
    h.button("Sign in to provider").await?;
    Ok(())
}
