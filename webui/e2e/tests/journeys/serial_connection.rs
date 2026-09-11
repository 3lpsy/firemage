use anyhow::Result;
use firemage_webui_e2e::Harness;
use serde_json::json;
use thirtyfour::prelude::*;

pub async fn connection(h: &Harness, id: &str, screenshot: &str) -> Result<()> {
    h.driver.execute(
        r#"const original = window.fetch;
        window.__serialConnection = {original, fail: true};
        const path = arguments[0];
        window.fetch = function(input, init) {
            const url = new URL(typeof input === 'string' ? input : input.url, location.origin);
            if (url.pathname === path && (init?.method || input.method || 'GET') === 'GET' && window.__serialConnection.fail) {
                window.__serialConnection.fail = false;
                return Promise.resolve(new Response(JSON.stringify({error: 'Temporary console failure'}), {status: 503, headers: {'Content-Type': 'application/json'}}));
            }
            return original.call(this, input, init);
        };"#,
        vec![json!(format!("/v1/vms/{id}/terminal"))],
    ).await?;
    h.button("TTY Stream").await?;
    h.text("Connection failed").await?;
    h.element(By::XPath(
        "//div[contains(@class,'serial-toolbar')]//button[normalize-space(.)='Reconnect']",
    ))
    .await?
    .click()
    .await?;
    h.text("Waiting for the VM to run.").await?;
    h.element(By::Css(".guest-terminal .xterm-fg-1")).await?;
    h.screenshot(screenshot).await?;

    h.driver
        .execute("window.__serialConnection.fail = true", vec![])
        .await?;
    h.text("Connection failed").await?;
    h.element(By::Css(".guest-terminal .xterm-fg-1")).await?;
    h.screenshot(&format!("{screenshot}-reconnect")).await?;
    h.button("Reconnect").await?;
    h.text("Waiting for the VM to run.").await?;
    h.button("Disconnect").await?;
    h.element(By::Css(".guest-terminal .xterm-fg-1")).await?;
    h.text("Disconnected").await?;
    h.button("Connect").await?;
    h.text("Waiting for the VM to run.").await?;
    h.button("Serial output").await?;
    h.absent(By::Css(".guest-terminal .xterm-screen")).await?;
    h.element(By::XPath(
        "//div[contains(@class,'serial-toolbar')]//button[normalize-space(.)='Refresh']",
    ))
    .await?
    .click()
    .await?;
    h.text("guest-console-ready").await?;
    h.driver
        .execute(
            "window.fetch = window.__serialConnection.original; delete window.__serialConnection",
            vec![],
        )
        .await?;
    Ok(())
}
