//! Copied boot files and post-setup userdata for a selected VM.
mod editor;
mod files;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Boot(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut editing = use_signal(|| false);
    let can_edit = matches!(vm["state"].as_str(), Some("defined" | "stopped" | "failed"));
    rsx! {
        h3 { "Boot files and userdata"
            Info { title: "Guest initialization",
                p { "Boot files are copied into the guest at their configured destinations with numeric owner, group, and Unix permissions. These are copies, never host filesystem shares." }
                p { "OCI guest setup prepares networking, loads /firemage/input/firemage/environment.sh, copies files using setup.sh, then runs /bin/sh /firemage/input/user-data before the image command. Custom VM images must implement this seed initialization sequence; simply attaching a seed disk does not run scripts." }
            }
        }
        for file in vm["spec"]["files"].as_array().into_iter().flatten() {
            div { class: "egress-summary-rule",
                strong { class: "mono", r#"{file["destination"].as_str().or(file["path"].as_str()).unwrap_or_default()}"# }
                span { class: "small muted", {format!("{}:{} · {:04o}", file["uid"].as_u64().unwrap_or(0), file["gid"].as_u64().unwrap_or(0), file["mode"].as_u64().unwrap_or(420))} }
            }
        }
        if vm["spec"]["files"].as_array().is_none_or(Vec::is_empty) { p { class: "small muted", "No boot files configured." } }
        h3 { class: "egress-section", "Userdata script" }
        if let Some(script) = vm["spec"]["userdata"].as_str() {
            pre { class: "console", "{script}" }
        } else { p { class: "small muted", "No userdata script configured." } }
        if auth.is_admin() {
            button { class: "primary", disabled: !can_edit, onclick: move |_| editing.set(true), "Configure boot inputs" }
            p { class: "small muted", "Stop the VM before changing boot inputs." }
        }
        if editing() {
            editor::BootEditor { vm: vm.clone(), onclose: move |_| editing.set(false), onsaved: move |_| { editing.set(false); onchanged.call(()); } }
        }
    }
}
