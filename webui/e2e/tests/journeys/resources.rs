use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use thirtyfour::prelude::*;

pub async fn resources(h: &Harness) -> Result<()> {
    h.login("admin").await?;
    h.navigate("Networks").await?;
    h.element(By::Css("button[aria-label='About Network policies']"))
        .await?
        .click()
        .await?;
    h.text("Specific host IP permits one host IP address")
        .await?;
    h.screenshot("network-help").await?;
    h.element(By::Css("button[aria-label='Close dialog']"))
        .await?
        .click()
        .await?;
    h.button("+ Create network").await?;
    h.fill("network-name", "harness-net").await?;
    h.fill("network-subnet", "172.29.0.0/24").await?;
    anyhow::ensure!(
        h.element(By::Id("network-gateway"))
            .await?
            .value()
            .await?
            .as_deref()
            == Some("172.29.0.1"),
        "gateway did not follow the entered subnet"
    );
    h.radio("network-policy", "Specific host IP").await?;
    h.fill("network-allowed", "192.0.2.10").await?;
    h.modal_button("Save network").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("harness-net").await?;
    h.text("192.0.2.10").await?;
    let networks = h.api("/v1/networks").await?;
    anyhow::ensure!(
        networks[0]["policy"]["mode"] == "host-only",
        "network policy was not persisted"
    );
    h.row_button("harness-net", "Edit").await?;
    h.radio("network-policy", "Isolated").await?;
    h.modal_button("Save network").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.element(By::Css(".status-isolated")).await?;
    h.screenshot("network-edited").await?;
    h.row_button("harness-net", "Delete").await?;
    h.modal_button("Delete network").await?;
    h.text("No networks defined").await?;
    anyhow::ensure!(
        h.api("/v1/networks")
            .await?
            .as_array()
            .context("networks response")?
            .is_empty(),
        "deleted network remains in database"
    );

    h.navigate("Virtual machines").await?;
    h.button("+ Create VM").await?;
    anyhow::ensure!(
        h.element(By::Css("input[name='vm-isolation'][value='jailed']"))
            .await?
            .is_selected()
            .await?,
        "new VMs must default to jailed host isolation"
    );
    h.fill("vm-name", "offline-harness").await?;
    h.select_kernel("vmlinux").await?;
    h.fill("vm-rootfs", &h.asset("rootfs.ext4")).await?;
    h.fill("vm-cpus", "2").await?;
    h.fill("vm-memory", "512").await?;
    h.button("Boot inputs").await?;
    h.fill("vm-userdata", "hello from the browser").await?;
    h.button("Network").await?;
    anyhow::ensure!(
        h.value("vm-network").await?.is_empty(),
        "new VMs must default to no networking"
    );
    h.radio("vm-isolation", "Trusted host process").await?;
    h.button("Create VM").await?;
    h.text("trusted VMs are disabled by host policy").await?;
    h.radio("vm-isolation", "Jailed").await?;
    h.screenshot("vm-create").await?;
    h.button("Create VM").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    h.navigate("Virtual machines").await?;
    h.element(By::Css(".vm-row td:nth-child(2)"))
        .await?
        .click()
        .await?;
    h.element(By::Css(".vm-detail")).await?;
    h.element(By::Css("button[aria-label='Close VM details']"))
        .await?
        .click()
        .await?;
    h.absent(By::Css(".vm-detail")).await?;
    for key in [Key::Enter, Key::Space] {
        h.element(By::Css(".vm-row .table-link"))
            .await?
            .send_keys(key)
            .await?;
        h.element(By::Css(".vm-row .table-link[aria-expanded='true']"))
            .await?;
        h.element(By::Css("button[aria-label='Close VM details']"))
            .await?
            .click()
            .await?;
        h.absent(By::Css(".vm-detail")).await?;
    }
    h.element(By::Css(".vm-row-chevron svg"))
        .await?
        .click()
        .await?;
    h.text("No network").await?;
    super::security::limits(h).await?;
    h.button("Overview").await?;
    h.button("Configure VM").await?;
    h.button("Full TOML").await?;
    let mut spec: toml::Value = toml::from_str(&h.value("vm-toml").await?)?;
    let fields = spec.as_table_mut().context("VM TOML must be a table")?;
    fields.insert("name".into(), "offline-edited".into());
    fields.insert("memory_mib".into(), 768.into());
    h.fill("vm-toml", &toml::to_string_pretty(&spec)?).await?;
    h.button("Save configuration").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    h.navigate("Virtual machines").await?;
    h.button("offline-edited").await?;
    h.text("768 MiB").await?;
    let vms = h.api("/v1/vms").await?;
    anyhow::ensure!(
        vms[0]["spec"]["network"].is_null()
            && vms[0]["spec"]["userdata"] == "hello from the browser",
        "VM editing lost offline/userdata configuration"
    );
    let vm_id = vms[0]["id"].as_str().context("VM id")?;
    super::serial::serial(h, vm_id, "sidebar-terminal").await?;
    h.element(By::Css(format!("a[href='#vms/{vm_id}']")))
        .await?
        .click()
        .await?;
    h.element(By::Css(".vm-full-page")).await?;
    super::serial::serial(h, vm_id, "fullpage-terminal").await?;
    h.element(By::Css("button[aria-label='Close VM details']"))
        .await?
        .click()
        .await?;
    h.button("offline-edited").await?;
    h.button("Start").await?;
    h.element(By::Css(".vm-detail .status-failed")).await?;
    h.element(By::Css(".vm-detail [role='alert']")).await?;
    h.screenshot("start-error").await?;
    h.button("Delete VM").await?;
    h.modal_button("Delete VM").await?;
    h.text("No virtual machines yet").await?;
    anyhow::ensure!(
        h.api("/v1/vms")
            .await?
            .as_array()
            .context("VM response")?
            .is_empty(),
        "deleted VM remains in database"
    );
    super::registry::registry(h).await?;
    h.navigate("Activity").await?;
    h.text("vm.delete").await?;
    h.screenshot("activity").await?;
    Ok(())
}
