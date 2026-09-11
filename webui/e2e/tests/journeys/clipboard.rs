use anyhow::Result;
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

pub async fn copy(h: &Harness, label: &str, expected: &str) -> Result<()> {
    h.driver.execute(r#"
        window.__copiedText = null;
        Object.defineProperty(navigator, 'clipboard', {configurable:true, value:{
            writeText: async text => { window.__copiedText = text; },
            write: async items => { window.__copiedText = await (await items[0].getType('text/plain')).text(); }
        }});
    "#, vec![]).await?;
    h.element(By::Css(format!("button[aria-label='{label}']")))
        .await?
        .click()
        .await?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let copied = h
            .driver
            .execute("return window.__copiedText", vec![])
            .await?;
        if let Some(copied) = copied.json().as_str() {
            anyhow::ensure!(
                copied == expected,
                "clipboard content did not match {label}"
            );
            return Ok(());
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "copy did not complete: {label}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
