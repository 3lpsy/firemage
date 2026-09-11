use anyhow::Result;
use firemage_webui_e2e::Harness;
use serde_json::json;
use thirtyfour::prelude::*;

pub async fn connection(h: &Harness, id: &str) -> Result<()> {
    h.button("Web Shell").await?;
    h.text("Start the VM to open a shell.").await?;
    h.driver
        .execute(include_str!("web_shell_fixture.js"), vec![json!(id)])
        .await?;
    h.navigate("Virtual machines").await?;
    h.driver.goto(format!("{}#vms/{id}", h.url)).await?;
    h.button("Web Shell").await?;
    h.text("Connection failed").await?;
    h.button("Reconnect").await?;
    h.text("Connected").await?;
    h.element(By::Css(".web-shell-terminal .xterm-fg-1"))
        .await?;
    h.driver
        .execute(
            "document.querySelector('.web-shell-terminal textarea').focus()",
            vec![],
        )
        .await?;
    h.driver
        .action_chain()
        .send_keys("echo shell\n")
        .perform()
        .await?;
    let frames = h
        .driver
        .execute("return window.__webShell.sent", vec![])
        .await?
        .convert::<serde_json::Value>()?;
    anyhow::ensure!(
        frames
            .as_array()
            .unwrap()
            .iter()
            .any(|frame| frame["type"] == "input"),
        "shell input was not sent"
    );
    anyhow::ensure!(
        frames
            .as_array()
            .unwrap()
            .iter()
            .any(|frame| frame["type"] == "resize"),
        "shell size was not sent"
    );
    h.screenshot("web-shell-connected").await?;
    h.button("Disconnect").await?;
    h.text("Disconnected").await?;
    h.element(By::Css(".web-shell-terminal .xterm-fg-1"))
        .await?;
    pending_connection(h, false).await?;
    h.button("Reconnect").await?;
    h.text("Connected").await?;
    h.button("Disconnect").await?;
    pending_connection(h, true).await?;
    h.absent(By::Css(".web-shell-terminal")).await?;
    let closed = h
        .driver
        .execute(
            "return window.__webShell.sockets.every(socket => socket.closed)",
            vec![],
        )
        .await?
        .convert::<bool>()?;
    anyhow::ensure!(closed, "leaving Web Shell did not close its connection");
    h.driver.execute("window.fetch = window.__webShell.original; window.WebSocket = window.__webShell.OriginalSocket; delete window.__webShell", vec![]).await?;
    Ok(())
}

async fn pending_connection(h: &Harness, leave_tab: bool) -> Result<()> {
    h.driver
        .execute("window.__webShell.deferNext = true", vec![])
        .await?;
    h.button("Reconnect").await?;
    h.text("Connecting…").await?;
    if leave_tab {
        h.button("Overview").await?;
    } else {
        h.button("Disconnect").await?;
        h.text("Disconnected").await?;
    }
    let safe = h.driver.execute_async(
        "const done = arguments[arguments.length - 1], fixture = window.__webShell, count = fixture.sockets.length; if (!fixture.resolvePending) throw Error('No pending shell request'); fixture.resolvePending(); requestAnimationFrame(() => requestAnimationFrame(() => done(fixture.pendingSignal.aborted && fixture.sockets.length === count)));",
        vec![],
    ).await?.convert::<bool>()?;
    anyhow::ensure!(safe, "a cancelled shell request created a late socket");
    Ok(())
}
