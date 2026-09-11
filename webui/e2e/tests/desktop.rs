mod journeys;
use firemage_webui_e2e::Harness;

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn kernel_catalog_and_vm_selection() -> anyhow::Result<()> {
    let harness = Harness::new("kernels").await?;
    let result = Harness::guarded(journeys::kernels(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn vm_and_network_management() -> anyhow::Result<()> {
    let harness = Harness::new("vm-network").await?;
    let result = Harness::guarded(journeys::resources(&harness)).await;
    harness.complete(result).await
}
#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn users_tokens_and_session_boundaries() -> anyhow::Result<()> {
    let harness = Harness::new("access").await?;
    let result = Harness::guarded(journeys::access(&harness)).await;
    harness.complete(result).await
}
#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn configuration_navigation_and_conflicts() -> anyhow::Result<()> {
    let harness = Harness::new("configuration").await?;
    let result = Harness::guarded(journeys::configuration(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn oidc_provider_login_and_account_binding() -> anyhow::Result<()> {
    let harness = Harness::oidc("oidc").await?;
    let result = Harness::guarded(journeys::oidc(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn oidc_confidential_client_authentication() -> anyhow::Result<()> {
    let harness = Harness::oidc_confidential("oidc-confidential").await?;
    let result = Harness::guarded(journeys::oidc(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn egress_secrets_and_boot_inputs() -> anyhow::Result<()> {
    let harness = Harness::new("egress").await?;
    let result = Harness::guarded(journeys::egress(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn asset_library_and_vm_attachments() -> anyhow::Result<()> {
    let harness = Harness::new("assets").await?;
    let result = Harness::guarded(journeys::assets(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn vm_create_and_edit_complete_configuration() -> anyhow::Result<()> {
    let harness = Harness::new("vm-form").await?;
    let result = Harness::guarded(journeys::vm_form(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn vm_export_import_and_duplicate() -> anyhow::Result<()> {
    let harness = Harness::new("vm-transfer").await?;
    let result = Harness::guarded(journeys::transfer(&harness)).await;
    harness.complete(result).await
}

#[tokio::test]
#[ignore = "requires the built UI, Firemage binary, Chromium and chromedriver; run just test-e2e"]
async fn snapshot_catalog_and_guest_files() -> anyhow::Result<()> {
    let harness = Harness::new("snapshots").await?;
    let result = Harness::guarded(journeys::snapshots(&harness)).await;
    harness.complete(result).await
}
