//! VM environment editor with explicit plain values and secret references.
mod editor;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Environment(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut editing = use_signal(|| false);
    let can_edit = matches!(vm["state"].as_str(), Some("defined" | "stopped" | "failed"));
    rsx! {
        h3 { "Guest environment"
            Info { title: "Environment and sensitive values",
                p { "Plain values are visible in the VM configuration. Sensitive entries refer to a named secret, which Firemage resolves when preparing the guest. The guest receives the actual value and can read it." }
                p { "Use HTTP header injection or request signing to keep an API credential outside the guest. Environment variables are appropriate when the guest application must have the credential itself." }
            }
        }
        if vm["spec"]["environment"].as_object().is_none_or(serde_json::Map::is_empty) {
            p { class: "small muted", "No environment variables configured." }
        }
        for (name, value) in vm["spec"]["environment"].as_object().into_iter().flatten() {
            div { class: "egress-summary-rule",
                strong { class: "mono", "{name}" }
                if let Some(secret) = value["secret"].as_str() {
                    span { class: "small purple", "Secret: {secret}" }
                } else {
                    code { "{value.as_str().unwrap_or_default()}" }
                }
            }
        }
        if auth.is_admin() {
            button { class: "primary egress-section", disabled: !can_edit, onclick: move |_| editing.set(true), "Configure environment" }
            p { class: "small muted", "Stop the VM before changing its environment." }
        }
        if editing() {
            editor::EnvironmentEditor { vm: vm.clone(), onclose: move |_| editing.set(false), onsaved: move |_| { editing.set(false); onchanged.call(()); } }
        }
    }
}
