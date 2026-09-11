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

// Persist capture-shaped records through the ORM; real guest capture is covered by FlanForge.
pub async fn inventory(h: &Harness, vm_id: &str, other_vm: &str, original_id: &str) -> Result<()> {
    let config: firemage_config::Config =
        toml::from_str(&std::fs::read_to_string(&h.config_path)?)?;
    let db =
        firemage_queries::connect(config.server.database.as_deref().expect("fixture database"))
            .await?;
    let original = firemage_queries::snapshot_by_id(&db, original_id).await?;
    let saved_id = "00000000-0000-4000-8000-000000000001";
    let other_id = "00000000-0000-4000-8000-000000000002";
    for (id, source, alias) in [
        (saved_id, vm_id, "saved-from-this-vm"),
        (other_id, other_vm, "saved-from-other-vm"),
    ] {
        let mut row = original.clone();
        row.id = id.into();
        row.alias = alias.into();
        row.source_vm_id = Some(source.into());
        row.source_vm_name = "snapshot-target".into();
        std::fs::copy(
            config
                .server
                .snapshot_dir()
                .join(format!("{original_id}.fmsnap")),
            config.server.snapshot_dir().join(format!("{id}.fmsnap")),
        )?;
        firemage_queries::insert_snapshot(&db, row).await?;
    }
    db.close().await?;
    h.navigate("Virtual machines").await?;
    h.element(By::Css(format!("a[href='#vms/{vm_id}/overview']")))
        .await?
        .click()
        .await?;
    h.element(By::XPath(
        "//*[@id='vm-detail']//button[normalize-space(.)='Snapshots']",
    ))
    .await?
    .click()
    .await?;
    let list = h
        .element(By::Css(".snapshot-vm-controls .snapshot-table"))
        .await?;
    let contents = list.text().await?;
    anyhow::ensure!(
        contents.contains("saved-from-this-vm")
            && !contents.contains("saved-from-other-vm")
            && !contents.contains("imported-checkpoint"),
        "VM snapshot inventory ignored source IDs: {contents}"
    );
    let layout = h.driver.execute("const section=document.querySelector('.snapshot-vm-controls'); const heading=section.querySelector('.heading'); return {border:getComputedStyle(section).borderTopWidth, actions:Array.from(heading.querySelectorAll('button')).map(button=>button.textContent.trim())};", vec![]).await?;
    anyhow::ensure!(
        layout.json()["border"] == "0px"
            && layout.json()["actions"]
                .as_array()
                .is_some_and(|actions| actions.contains(&json!("Save snapshot"))
                    && actions.contains(&json!("Restore snapshot"))),
        "snapshot actions are not inline or retain double border: {:?}",
        layout.json()
    );
    h.screenshot("vm-snapshot-inventory").await?;
    h.element(By::LinkText("saved-from-this-vm"))
        .await?
        .click()
        .await?;
    h.element(By::Css(".snapshot-drawer")).await?;
    anyhow::ensure!(
        h.driver.current_url().await?.fragment() == Some(&format!("snapshots/{saved_id}")),
        "snapshot alias did not open its inspector route"
    );
    let link = h.element(By::LinkText("saved-from-this-vm")).await?;
    anyhow::ensure!(
        link.attr("href")
            .await?
            .is_some_and(|href| href.ends_with(&format!("#snapshots/{saved_id}"))),
        "selected snapshot alias lost its permalink"
    );
    h.element(By::Css(".snapshot-vm-picker .kernel-trigger"))
        .await?
        .click()
        .await?;
    let search = h
        .element(By::Css(".snapshot-vm-picker input[role='combobox']"))
        .await?;
    search.send_keys(vm_id).await?;
    search.send_keys(Key::Down + Key::Enter).await?;
    let trust = h
        .element(By::Css(".snapshot-restore input[type='checkbox']"))
        .await?;
    super::switches::keyboard_toggle(&trust).await?;
    h.element(By::LinkText("saved-from-other-vm"))
        .await?
        .click()
        .await?;
    h.element(By::XPath(
        "//aside[@aria-label='Snapshot details']//h2[normalize-space()='saved-from-other-vm']",
    ))
    .await?;
    anyhow::ensure!(
        !h.element(By::Css(".snapshot-restore input[type='checkbox']"))
            .await?
            .is_selected()
            .await?,
        "switching snapshots retained trust acknowledgement"
    );
    anyhow::ensure!(
        h.element(By::Css(".snapshot-vm-picker .kernel-trigger"))
            .await?
            .text()
            .await?
            == "Choose a compatible VM",
        "switching snapshots retained the previous restore target"
    );
    h.element(By::LinkText("saved-from-this-vm"))
        .await?
        .click()
        .await?;
    h.element(By::XPath(
        "//aside[@aria-label='Snapshot details']//h2[normalize-space()='saved-from-this-vm']",
    ))
    .await?;
    h.element(By::Css("button[aria-label='Close snapshot details']"))
        .await?
        .click()
        .await?;
    h.absent(By::Css(".snapshot-drawer")).await?;
    h.element(By::Css(
        "button[aria-label='Toggle saved-from-this-vm details']",
    ))
    .await?
    .click()
    .await?;
    h.element(By::Css(".snapshot-drawer")).await?;
    h.element(By::Css(
        "button[aria-label='Toggle saved-from-this-vm details']",
    ))
    .await?
    .send_keys(Key::Enter)
    .await?;
    h.absent(By::Css(".snapshot-drawer")).await?;
    h.element(By::XPath("//table[contains(@class,'snapshot-table')]//tr[.//a[normalize-space()='saved-from-this-vm']]/td[2]")).await?.click().await?;
    h.element(By::Css(".snapshot-drawer")).await?;
    h.driver.refresh().await?;
    h.element(By::Css(".snapshot-drawer")).await?;
    anyhow::ensure!(
        h.element(By::Css(".snapshot-drawer h2"))
            .await?
            .text()
            .await?
            == "saved-from-this-vm",
        "refresh lost the selected snapshot"
    );
    for id in [saved_id, other_id] {
        h.client
            .delete(format!("{}/v1/snapshots/{id}", h.url))
            .bearer_auth(&h.token)
            .send()
            .await?
            .error_for_status()?;
    }
    h.navigate("Snapshots").await?;
    h.element(By::Css("button[aria-label='Refresh snapshots']"))
        .await?
        .click()
        .await?;
    h.element(By::LinkText("imported-checkpoint"))
        .await?
        .click()
        .await?;
    h.element(By::Css(".snapshot-drawer")).await?;
    Ok(())
}
