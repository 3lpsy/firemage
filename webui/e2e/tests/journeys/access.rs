use anyhow::{Context, Result};
use firemage_webui_e2e::{Harness, PASSWORD};
use thirtyfour::prelude::*;

pub async fn access(h: &Harness) -> Result<()> {
    h.fill("login-username", "admin").await?;
    h.fill("login-password", "wrong-password").await?;
    h.button("Sign in").await?;
    h.text("invalid username or password").await?;
    h.login("admin").await?;
    let cookies = h
        .driver
        .execute("return document.cookie", vec![])
        .await?
        .json()
        .as_str()
        .unwrap_or_default()
        .to_owned();
    anyhow::ensure!(
        !cookies.contains("firemage_session"),
        "session cookie is readable by page JavaScript"
    );
    h.driver.refresh().await?;
    h.element(By::Css(".app-shell")).await?;
    h.navigate("Users").await?;
    h.button("+ Create user").await?;
    h.fill("user-username", "reader").await?;
    h.fill("user-password", PASSWORD).await?;
    h.radio("user-role", "Member").await?;
    h.modal_button("Save user").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("reader").await?;
    h.screenshot("users").await?;
    h.logout().await?;
    h.login("reader").await?;
    h.absent(By::XPath("//nav//button[contains(.,'Users')]"))
        .await?;
    h.absent(By::XPath("//nav//button[contains(.,'Configuration')]"))
        .await?;
    h.absent(By::XPath("//button[normalize-space(.)='+ Create VM']"))
        .await?;
    h.navigate("API tokens").await?;
    let token = create_token(h, "browser-runner").await?;
    let me = h
        .client
        .get(format!("{}/v1/me", h.url))
        .bearer_auth(&token)
        .send()
        .await?;
    anyhow::ensure!(
        me.status().is_success() && me.json::<serde_json::Value>().await?["username"] == "reader",
        "created token does not authenticate as its owner"
    );
    for path in ["/v1/users", "/v1/config"] {
        let response = h
            .client
            .get(format!("{}{path}", h.url))
            .bearer_auth(&token)
            .send()
            .await?;
        anyhow::ensure!(
            response.status() == reqwest::StatusCode::FORBIDDEN,
            "member can access {path}"
        );
    }
    h.row_button("browser-runner", "Revoke").await?;
    h.modal_button("Revoke token").await?;
    h.text("No API tokens").await?;
    anyhow::ensure!(
        h.client
            .get(format!("{}/v1/me", h.url))
            .bearer_auth(&token)
            .send()
            .await?
            .status()
            == reqwest::StatusCode::UNAUTHORIZED,
        "revoked token still authenticates"
    );
    let pending = create_token(h, "disable-test").await?;
    h.screenshot("member-tokens").await?;
    h.logout().await?;
    h.login("admin").await?;
    h.navigate("Users").await?;
    h.row_button("reader", "Edit").await?;
    h.radio("user-status", "Disabled").await?;
    h.modal_button("Save user").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.element(By::Css(".status-disabled")).await?;
    anyhow::ensure!(
        h.client
            .get(format!("{}/v1/me", h.url))
            .bearer_auth(&pending)
            .send()
            .await?
            .status()
            == reqwest::StatusCode::UNAUTHORIZED,
        "disabled user's token still authenticates"
    );
    h.row_button("admin", "Edit").await?;
    h.radio("user-status", "Disabled").await?;
    h.modal_button("Save user").await?;
    h.text("cannot remove the last enabled administrator")
        .await?;
    h.screenshot("last-admin-protected").await?;
    h.element(By::Css("button[aria-label='Close dialog']"))
        .await?
        .click()
        .await?;
    h.logout().await?;
    h.fill("login-username", "reader").await?;
    h.fill("login-password", PASSWORD).await?;
    h.button("Sign in").await?;
    h.text("invalid username or password").await?;
    h.login("admin").await?;
    h.navigate("Users").await?;
    h.row_button("reader", "Delete").await?;
    h.modal_button("Delete user").await?;
    h.absent(By::XPath("//tr[.//*[normalize-space(.)='reader']]"))
        .await?;
    anyhow::ensure!(
        h.api("/v1/users")
            .await?
            .as_array()
            .context("user response")?
            .len()
            == 1,
        "user deletion did not persist"
    );
    h.logout().await?;
    Ok(())
}
async fn create_token(h: &Harness, name: &str) -> Result<String> {
    h.button("+ Create API token").await?;
    h.fill("token-name", name).await?;
    h.modal_button("Create token").await?;
    let token = h.value("created-token").await?;
    anyhow::ensure!(
        token.starts_with("fm_api_"),
        "created token is missing its credential prefix"
    );
    h.modal_button("I have saved the token").await?;
    h.absent(By::Id("created-token")).await?;
    h.text(name).await?;
    Ok(token)
}
