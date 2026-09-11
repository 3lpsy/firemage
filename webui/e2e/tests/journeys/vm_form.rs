use anyhow::{Context, Result};
use firemage_webui_e2e::Harness;
use serde_json::{Value, json};
use thirtyfour::{components::SelectElement, prelude::*};

pub async fn vm_form(h: &Harness) -> Result<()> {
    setup(h).await?;
    h.login("admin").await?;
    h.navigate("Virtual machines").await?;
    h.button("+ Create VM").await?;
    h.element(By::Css(".vm-editor-page")).await?;
    anyhow::ensure!(
        h.driver.current_url().await?.fragment() == Some("vms/new"),
        "create is not addressable"
    );
    h.absent(By::Css("[role='dialog']")).await?;
    h.fill("vm-name", "complete-form").await?;
    h.fill("vm-cpus", "2").await?;
    h.fill("vm-memory", "512").await?;
    h.select_kernel("vmlinux").await?;
    anyhow::ensure!(
        h.element(By::Css("input[name='asset-source'][value='oci']"))
            .await?
            .is_selected()
            .await?,
        "OCI is not the default image source"
    );
    h.fill("vm-rootfs", "alpine:3.22").await?;
    h.fill("vm-rootfs-size", "8192").await?;
    h.button("Configure initrd and additional drives").await?;
    h.fill("vm-storage-toml", &format!(
        "[initrd]\nkind = \"local\"\npath = {:?}\n\n[[drives]]\nid = \"data\"\nread_only = true\n[drives.asset]\nkind = \"local\"\npath = {:?}\n",
        h.asset("initrd"), h.asset("data.ext4"),
    )).await?;
    apply(h).await?;
    h.button("Workload").await?;
    h.radio("workload-mode", "Keep alive").await?;
    h.fill(
        "vm-workload-command",
        r#"["/bin/sh", "-lc", "echo review"]"#,
    )
    .await?;
    h.button("Network").await?;
    h.select_network("form-net").await?;
    for _ in 0..50 {
        if h.value("vm-address").await? == "10.77.0.2" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    anyhow::ensure!(
        h.value("vm-address").await? == "10.77.0.2",
        "guest address was not suggested"
    );
    let mac = h.value("vm-mac").await?;
    anyhow::ensure!(!mac.is_empty(), "MAC was not suggested");
    h.element(By::Css("button[aria-controls='vm-section-egress']"))
        .await?
        .click()
        .await?;
    h.element(By::Css("button[aria-label='Create Egress policy']"))
        .await?
        .click()
        .await?;
    h.fill("policy-alias", "form-policy").await?;
    h.element(By::Id("egress-http-enabled"))
        .await?
        .click()
        .await?;
    h.button("+ Add HTTP rule").await?;
    h.fill("egress-rule-0-host", "model.example.com").await?;
    h.fill("egress-rule-0-path_prefix", "/v1/").await?;
    h.button("Create and select").await?;
    h.element(By::Id("vm-name")).await?;
    h.fill("vm-memory", "").await?;
    h.button("Environment").await?;
    h.button("Configure environment").await?;
    h.button("+ Add variable").await?;
    h.fill("environment-0-name", "REGION").await?;
    h.fill("environment-0-literal", "test-region").await?;
    h.button("+ Add variable").await?;
    h.fill("environment-1-name", "MODEL_KEY").await?;
    h.radio("environment-1-mode", "Sensitive secret").await?;
    SelectElement::new(&h.element(By::Id("environment-1-secret")).await?)
        .await?
        .select_by_value("form-secret")
        .await?;
    h.screenshot("vm-form-environment-draft").await?;
    apply(h).await?;
    anyhow::ensure!(
        h.value("vm-memory").await?.is_empty(),
        "applying a section silently replaced unfinished capacity"
    );
    h.fill("vm-memory", "512").await?;
    anyhow::ensure!(
        h.api("/v1/vms").await?.as_array().unwrap().is_empty(),
        "section editor saved a partial VM"
    );
    attach(h).await?;
    h.button("Boot inputs").await?;
    h.fill(
        "vm-boot-args",
        "console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw",
    )
    .await?;
    super::userdata::editor(h, "vm-userdata").await?;
    h.fill("vm-userdata", "echo setup").await?;
    h.button("Configure boot inputs").await?;
    h.button("+ Add boot file").await?;
    h.fill("boot-0-path", "instructions").await?;
    h.fill("boot-0-destination", "/etc/reviewer/instructions")
        .await?;
    h.fill("boot-0-content", "Review for bugs").await?;
    apply(h).await?;
    h.button("Security limits").await?;
    h.button("Configure host limits").await?;
    h.fill("security-pids_max", "200").await?;
    apply(h).await?;
    h.button("Metadata").await?;
    h.fill("vm-metadata", r#"{"job":"review"}"#).await?;
    h.button("Terminal").await?;
    let terminal = h.element(By::Id("vm-terminal")).await?;
    super::switches::keyboard_toggle(&terminal).await?;
    h.screenshot("guest-serial-input-switch").await?;
    let shell = h.element(By::Id("vm-web-terminal")).await?;
    h.element(By::Css("button[aria-label='About Web Terminal']"))
        .await?
        .click()
        .await?;
    h.text("Custom images must support the guest helper")
        .await?;
    h.driver
        .execute("document.querySelector('.modal-backdrop').click()", vec![])
        .await?;
    h.absent(By::Css("[role='dialog']")).await?;
    anyhow::ensure!(
        !shell.is_selected().await?,
        "dismissing Web Terminal info toggled its switch"
    );
    super::switches::keyboard_toggle(&shell).await?;
    h.fill("vm-shell-command", r#"["/bin/sh", "-i"]"#).await?;
    h.screenshot("web-terminal-settings").await?;
    h.button("Full TOML").await?;
    let draft = h.value("vm-toml").await?;
    anyhow::ensure!(
        draft.contains("keep-alive")
            && draft.contains("form-secret")
            && draft.contains("egress_policy"),
        "full TOML lost section drafts"
    );
    h.button("Guided setup").await?;
    h.element(By::Css(".vm-editor-page h1"))
        .await?
        .scroll_into_view()
        .await?;
    h.screenshot("vm-form-create-complete").await?;
    h.button("Create VM").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    let rows = h.api("/v1/vms").await?;
    anyhow::ensure!(
        rows.as_array().unwrap().len() == 1,
        "creation did not save exactly one VM"
    );
    let vm = &rows[0];
    let id = vm["id"].as_str().context("VM id")?;
    let spec = &vm["spec"];
    check(spec)?;
    let policy_id = spec["egress_policy"].as_str().context("shared policy")?;
    let policy = h.api(&format!("/v1/egress/policies/{policy_id}")).await?;
    anyhow::ensure!(
        policy["policy"]["http"]["rules"][0]["host"] == "model.example.com",
        "shared policy lost HTTP rules"
    );
    anyhow::ensure!(vm["state"] == "defined", "creation started the VM");
    anyhow::ensure!(
        spec["network"]["mac"] == mac,
        "suggested MAC changed on save"
    );
    anyhow::ensure!(
        h.driver.current_url().await?.fragment() == Some(&format!("vms/{id}/overview")),
        "save did not navigate to VM detail"
    );
    let network_id = spec["network"]["network"]
        .as_str()
        .context("network UUID")?;
    let networks = h.api("/v1/networks").await?;
    anyhow::ensure!(
        networks[0]["id"] == network_id,
        "VM did not retain network UUID"
    );
    h.navigate("Networks").await?;
    h.row_button("form-net", "Edit").await?;
    h.fill("network-name", "renamed-form-net").await?;
    h.modal_button("Save network").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.text("renamed-form-net").await?;
    let renamed = h.api("/v1/networks").await?;
    anyhow::ensure!(
        renamed[0]["id"] == network_id,
        "rename changed network identity"
    );
    let unchanged = h.api(&format!("/v1/vms/{id}")).await?;
    anyhow::ensure!(
        unchanged["spec"] == *spec,
        "rename changed VM configuration"
    );
    h.driver.goto(format!("{}#vms/{id}", h.url)).await?;
    h.element(By::Css(".vm-full-page")).await?;
    h.text("renamed-form-net").await?;
    h.screenshot("network-renamed-vm").await?;
    h.driver.goto(format!("{}#vms/{id}/edit", h.url)).await?;
    h.element(By::Css(".vm-editor-page")).await?;
    let runtime = h.driver.find(By::Css("input[name='runtime-mode']")).await?;
    anyhow::ensure!(!runtime.is_enabled().await?, "edit allowed runtime changes");
    h.button("Network").await?;
    h.element(By::Css(format!("#vm-network option[value='{network_id}']")))
        .await?;
    anyhow::ensure!(
        h.value("vm-network").await? == network_id,
        "edit lost the renamed network"
    );
    h.fill("vm-memory", "1024").await?;
    h.button("Environment").await?;
    h.button("Configure environment").await?;
    h.fill("environment-0-name", "SHOULD_NOT_SAVE").await?;
    // Alphabetical order places the secret first; cancellation must retain it.
    h.modal_button("Cancel").await?;
    h.absent(By::Css("[role='dialog']")).await?;
    h.button("Metadata").await?;
    anyhow::ensure!(
        h.value("vm-metadata").await?.contains("review"),
        "edit lost metadata"
    );
    h.screenshot("vm-form-edit-complete").await?;
    h.button("Save configuration").await?;
    h.absent(By::Css(".vm-editor-page")).await?;
    let updated = h.api(&format!("/v1/vms/{id}")).await?;
    check(&updated["spec"])?;
    anyhow::ensure!(
        updated["spec"]["memory_mib"] == 1024,
        "edit did not save memory"
    );
    let mut expected = spec.clone();
    expected["memory_mib"] = json!(1024);
    anyhow::ensure!(
        updated["spec"] == expected,
        "editing capacity lost another section"
    );
    super::web_shell::connection(h, id).await?;
    Ok(())
}

async fn apply(h: &Harness) -> Result<()> {
    h.modal_button("Apply to draft").await?;
    h.absent(By::Css("[role='dialog']")).await
}

async fn setup(h: &Harness) -> Result<()> {
    h.client.post(format!("{}/v1/networks", h.url)).bearer_auth(&h.token)
        .json(&json!({"name":"form-net","subnet":"10.77.0.0/24","gateway":"10.77.0.1","policy":{"mode":"firemage-only"}}))
        .send().await?.error_for_status()?;
    h.client
        .put(format!("{}/v1/secrets/form-secret", h.url))
        .bearer_auth(&h.token)
        .json(&json!({"value":"private-form-fixture"}))
        .send()
        .await?
        .error_for_status()?;
    h.client
        .post(format!(
            "{}/v1/assets?alias=form-source&filename=source.tar",
            h.url
        ))
        .bearer_auth(&h.token)
        .body("fixture source")
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn attach(h: &Harness) -> Result<()> {
    h.button("Attachments").await?;
    h.button("+ Attach asset").await?;
    h.element(By::Id("vm-asset-0")).await?.click().await?;
    h.fill("vm-asset-search-0", "form-source").await?;
    h.element(By::Id("vm-asset-search-0"))
        .await?
        .send_keys(Key::Down + Key::Enter)
        .await?;
    h.fill("asset-destination-0", "/workspace/source.tar")
        .await?;
    h.button("+ Attach asset").await?;
    SelectElement::new(&h.element(By::Id("attachment-source-1")).await?)
        .await?
        .select_by_value("secret")
        .await?;
    h.fill("vm-secret-file-1", "form-secret").await?;
    h.fill("asset-destination-1", "/etc/reviewer/key").await?;
    Ok(())
}

fn check(spec: &Value) -> Result<()> {
    anyhow::ensure!(
        spec["rootfs"]["size_mib"] == 8192 && spec["workload"]["mode"] == "keep-alive",
        "lost disk size or workload"
    );
    anyhow::ensure!(
        spec["workload"]["command"][2] == "echo review" && spec["terminal"] == true,
        "lost command or terminal"
    );
    anyhow::ensure!(
        spec["environment"]["MODEL_KEY"]["secret"] == "form-secret"
            && spec["environment"]["REGION"] == "test-region",
        "lost environment"
    );
    anyhow::ensure!(
        spec["attachments"][0]["destination"] == "/workspace/source.tar"
            && spec["secret_attachments"][0]["mode"] == 384,
        "lost attachments"
    );
    anyhow::ensure!(
        spec["web_terminal"]["command"] == json!(["/bin/sh", "-i"]),
        "lost web terminal command"
    );
    anyhow::ensure!(spec["egress_policy"].is_string(), "lost egress");
    anyhow::ensure!(
        spec["files"][0]["content"] == "Review for bugs" && spec["userdata"] == "echo setup",
        "lost boot inputs"
    );
    anyhow::ensure!(
        spec["security"]["pids_max"] == 200 && spec["metadata"]["job"] == "review",
        "lost security or metadata"
    );
    anyhow::ensure!(
        spec["initrd"]["kind"] == "local" && spec["drives"][0]["read_only"] == true,
        "lost storage"
    );
    anyhow::ensure!(
        !spec.to_string().contains("private-form-fixture"),
        "saved plaintext secret"
    );
    Ok(())
}
