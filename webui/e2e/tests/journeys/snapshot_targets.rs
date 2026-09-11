use anyhow::Result;
use firemage_webui_e2e::Harness;
use serde_json::{Value, json};
use thirtyfour::prelude::*;

// Mock capture transport; the real paused-guest capture runs in FlanForge.
pub async fn save(h: &Harness, targets: &[Value], choice: &str, id: &str) -> Result<()> {
    h.driver
        .execute(
            include_str!("snapshot_targets_fixture.js"),
            vec![json!(targets)],
        )
        .await?;
    h.button("Save VM snapshot").await?;
    let picker = h
        .element(By::Css("input[list='snapshot-save-vms']"))
        .await?;
    h.fill("snapshot-save-alias", "selected-target").await?;
    picker.send_keys("snapshot-target").await?;
    anyhow::ensure!(
        !h.driver
            .find(By::Css("[role='dialog'] button[type='submit']"))
            .await?
            .is_enabled()
            .await?,
        "capture accepted an ambiguous VM name"
    );
    picker.clear().await?;
    picker.send_keys(choice).await?;
    h.modal_button("Save snapshot").await?;
    h.text("Capture target recorded").await?;
    let path = h
        .driver
        .execute("return window.__snapshotTargets.path", vec![])
        .await?;
    anyhow::ensure!(
        path.json() == &json!(format!("/v1/vms/{id}/snapshots")),
        "capture selected another VM with the same name"
    );
    h.modal_button("Cancel").await?;
    h.driver
        .execute(
            "window.fetch = window.__snapshotTargets.original; delete window.__snapshotTargets",
            vec![],
        )
        .await?;
    Ok(())
}

pub async fn restore(h: &Harness, id: &str, snapshot: &str) -> Result<()> {
    h.driver
        .execute(
            include_str!("snapshot_targets_fixture.js"),
            vec![json!([]), json!("restore"), json!(snapshot)],
        )
        .await?;
    h.element(By::Css(".snapshot-restore button.primary"))
        .await?
        .click()
        .await?;
    h.modal_button("Restore snapshot").await?;
    h.text("Restore target recorded").await?;
    let path = h
        .driver
        .execute("return window.__snapshotTargets.path", vec![])
        .await?;
    anyhow::ensure!(
        path.json() == &json!(format!("/v1/vms/{id}/snapshots/restore")),
        "restore selected another VM with the same name"
    );
    h.driver
        .execute(
            "window.fetch = window.__snapshotTargets.original; delete window.__snapshotTargets",
            vec![],
        )
        .await?;
    Ok(())
}
