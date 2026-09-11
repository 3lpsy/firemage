use anyhow::Result;
use firemage_webui_e2e::Harness;
use serde_json::json;
use thirtyfour::prelude::*;

pub async fn install(h: &Harness, vm_id: &str) -> Result<()> {
    h.driver
        .execute(include_str!("guest_files_fixture.js"), vec![json!(vm_id)])
        .await?;
    Ok(())
}

// Browser-only fixtures exercise navigation; FlanForge covers real ext4 extraction.
pub async fn navigate(h: &Harness) -> Result<()> {
    anyhow::ensure!(
        reads(h).await?.is_empty(),
        "Files requested directories before the VM stopped"
    );
    h.driver
        .execute("window.__guestFiles.stopped = true", vec![])
        .await?;
    h.navigate("Networks").await?;
    h.navigate("Virtual machines").await?;
    h.button("snapshot-target").await?;
    h.button("Files").await?;
    h.element(By::Css(".guest-file-table tbody tr")).await?;
    anyhow::ensure!(
        reads(h).await? == vec![2],
        "initial render eagerly read child directories"
    );

    h.button("Directories").await?;
    h.element(By::Css(
        ".guest-tree-expand[aria-label='Expand review files']",
    ))
    .await?
    .click()
    .await?;
    h.element(By::XPath(
        "//table[contains(@class,'guest-file-table')]//a[contains(@class,'guest-folder-link')][normalize-space()='nested']",
    ))
    .await?
    .click()
    .await?;
    h.text("résumé notes.txt").await?;
    h.text("Exceeds download limit").await?;
    anyhow::ensure!(
        reads(h).await? == vec![2, 10, 20],
        "directory tree did not load only opened folders"
    );
    let download = h.element(By::Css(".guest-file-table a[download]")).await?;
    let href = download.attr("href").await?.unwrap_or_default();
    anyhow::ensure!(
        href.contains("/files/download?inode=31&filename=r%C3%A9sum%C3%A9%20notes.txt"),
        "download link lost the inode or Unicode filename: {href}"
    );
    anyhow::ensure!(
        h.driver
            .find_all(By::Css(".guest-file-table a[download]"))
            .await?
            .len()
            == 1,
        "symlinks or oversized files had download links"
    );
    h.screenshot("guest-files-tree").await?;
    h.element(By::Css(".guest-file-breadcrumbs a"))
        .await?
        .click()
        .await?;
    h.element(By::Css(".guest-file-table .guest-folder-link"))
        .await?;
    anyhow::ensure!(
        reads(h).await? == vec![2, 10, 20],
        "breadcrumb navigation discarded the directory cache"
    );
    h.driver
        .execute(
            "window.fetch = window.__guestFiles.original; delete window.__guestFiles",
            vec![],
        )
        .await?;
    Ok(())
}

async fn reads(h: &Harness) -> Result<Vec<u64>> {
    Ok(serde_json::from_value(
        h.driver
            .execute("return window.__guestFiles.reads", vec![])
            .await?
            .json()
            .clone(),
    )?)
}
