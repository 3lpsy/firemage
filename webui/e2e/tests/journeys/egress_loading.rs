use anyhow::{Result, ensure};
use firemage_webui_e2e::Harness;
use serde_json::{Value, json};
use std::time::Duration;
use thirtyfour::prelude::*;

// Hold actual browser requests to inspect loading and out-of-order completion deterministically.
pub async fn catalog_switching(h: &Harness) -> Result<()> {
    h.driver
        .execute(include_str!("egress_loading_fixture.js"), vec![])
        .await?;
    h.button("Egress policies").await?;
    pending(h, "/v1/egress/policies").await?;
    loading(h, "Egress policies").await?;

    h.button("Upstream proxies").await?;
    pending(h, "/v1/egress/proxies").await?;
    release(
        h,
        "/v1/egress/policies",
        json!([{ "id": "abandoned", "alias": "abandoned-policy" }]),
        200,
    )
    .await?;
    loading(h, "Upstream proxies").await?;
    release(h, "/v1/egress/proxies", Value::Null, 200).await?;
    h.button("shared-exit").await?;
    let proxies = h.api("/v1/egress/proxies").await?;
    let id = proxies[0]["id"].as_str().expect("proxy id");
    let detail = format!("/v1/egress/proxies/{id}");
    pending(h, &detail).await?;
    ensure!(
        h.driver
            .find_all(By::Css(".egress-catalog-drawer button"))
            .await?
            .is_empty(),
        "loading details retained actions from another resource"
    );
    release(h, &detail, Value::Null, 200).await?;
    h.text("System trust").await?;

    h.button("Egress policies").await?;
    pending(h, "/v1/egress/policies").await?;
    loading(h, "Egress policies").await?;
    release(
        h,
        "/v1/egress/policies",
        json!({"error":"Policy list unavailable"}),
        503,
    )
    .await?;
    h.text("Policy list unavailable").await?;
    ensure!(
        h.driver
            .find_all(By::Css(".egress-catalog-table, .egress-catalog-drawer"))
            .await?
            .is_empty(),
        "failed policy request retained upstream content"
    );
    h.button("Refresh egress").await?;
    pending(h, "/v1/egress/policies").await?;
    release(h, "/v1/egress/policies", Value::Null, 200).await?;
    h.element(By::XPath(
        "//table//button[normalize-space(.)='shared-review']",
    ))
    .await?;
    h.driver.execute(
        "window.fetch = window.__egressFetch.original; for (const request of window.__egressFetch.pending) request.release(null); delete window.__egressFetch",
        vec![],
    ).await?;
    Ok(())
}

async fn pending(h: &Harness, path: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let pending = h.driver.execute(
                "return window.__egressFetch.pending.some(request => request.path === arguments[0])",
                vec![json!(path)],
            ).await?;
            if pending.json() == &json!(true) { return Ok::<_, anyhow::Error>(()); }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await??;
    Ok(())
}

async fn release(h: &Harness, path: &str, body: Value, status: u16) -> Result<()> {
    h.driver.execute(
        "const pending = window.__egressFetch.pending; const index = pending.findIndex(request => request.path === arguments[0]); if (index < 0) throw Error('No pending request'); pending.splice(index, 1)[0].release(arguments[1], arguments[2]);",
        vec![json!(path), body, json!(status)],
    ).await?;
    Ok(())
}

async fn loading(h: &Harness, tab: &str) -> Result<()> {
    // Two rendered frames also let an incorrectly retained request commit its result.
    let state = h.driver.execute_async(
        "const done = arguments[arguments.length - 1]; requestAnimationFrame(() => requestAnimationFrame(() => done({tab: document.querySelector('.egress-catalog-page .tabs .active')?.textContent, content: document.querySelector('.egress-catalog')?.textContent, stale: document.querySelectorAll('.egress-catalog-table, .egress-catalog-drawer, [role=dialog]').length})));",
        vec![],
    ).await?;
    ensure!(
        state.json()["tab"] == tab,
        "wrong tab selected: {:?}",
        state.json()
    );
    ensure!(
        state.json()["stale"] == 0,
        "previous catalog content remained visible: {:?}",
        state.json()
    );
    ensure!(
        state.json()["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Loading egress"),
        "missing loading state: {:?}",
        state.json()
    );
    Ok(())
}
