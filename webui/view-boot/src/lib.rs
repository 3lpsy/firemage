//! Copied boot files and post-setup userdata for a selected VM.
mod editor;
pub use editor::BootEditor;
mod files;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Boot(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let assets = use_resource(|| async { firemage_webui_provider_api::get("/v1/assets").await });
    let mut editing = use_signal(|| false);
    let can_edit = matches!(vm["state"].as_str(), Some("defined" | "stopped" | "failed"));
    rsx! {
        div { class: "heading compact",
            h3 { "Boot files and userdata"
                Info { title: "Guest initialization",
                    p { "Boot files are copied into the guest at their configured destinations with numeric owner, group, and Unix permissions. These are copies, never host filesystem shares." }
                    p { "OCI guest setup prepares networking, loads /firemage/input/firemage/environment.sh, copies files using setup.sh, then runs /bin/sh /firemage/input/user-data before the image command. Custom VM images must implement this seed initialization sequence; simply attaching a seed disk does not run scripts." }
                }
            }
            if auth.is_admin() { button { disabled: !can_edit, onclick: move |_| editing.set(true), "Configure boot inputs" } }
        }
        for attachment in vm["spec"]["attachments"].as_array().into_iter().flatten() {
            div { class: "egress-summary-rule",
                strong { class: "mono", {attachment["destination"].as_str().unwrap_or_default()} }
                span { class: "small muted", {
                    assets.read().as_ref().and_then(|r| r.as_ref().ok()).and_then(|rows| rows.as_array()).and_then(|rows| rows.iter().find(|row| row["id"] == attachment["asset_id"]))
                        .and_then(|row| row["alias"].as_str()).unwrap_or("Library asset").to_owned()
                } }
            }
        }
        for file in vm["spec"]["files"].as_array().into_iter().flatten() {
            div { class: "egress-summary-rule",
                strong { class: "mono", r#"{file["destination"].as_str().or(file["path"].as_str()).unwrap_or_default()}"# }
                span { class: "small muted", {format!("{}:{} · {:04o}", file["uid"].as_u64().unwrap_or(0), file["gid"].as_u64().unwrap_or(0), file["mode"].as_u64().unwrap_or(420))} }
            }
        }
        if vm["spec"]["files"].as_array().is_none_or(Vec::is_empty) && vm["spec"]["attachments"].as_array().is_none_or(Vec::is_empty) { p { class: "small muted", "No boot files configured." } }
        h3 { class: "egress-section", "Userdata script" }
        if let Some(script) = vm["spec"]["userdata"].as_str() {
            pre { class: "console", "{script}" }
        } else { p { class: "small muted", "No userdata script configured." } }
        if auth.is_admin() {
            p { class: "small muted", "Stop the VM before changing boot inputs." }
        }
        if editing() {
            editor::BootEditor { vm: vm.clone(), onclose: move |_| editing.set(false), onsaved: move |_| { editing.set(false); onchanged.call(()); } }
        }
    }
}
