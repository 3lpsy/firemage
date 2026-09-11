mod logs;
mod terminal;

use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use firemage_webui_provider_api::request;
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Serial(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let id = vm["id"].as_str().unwrap_or_default().to_owned();
    let enabled = vm["spec"]["terminal"].as_bool().unwrap_or(false);
    let editable = matches!(vm["state"].as_str(), Some("defined" | "stopped" | "failed"));
    let mut terminal = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(String::new);
    let save_id = id.clone();
    rsx! {
        div { class: "heading compact",
            div { class: "tabs",
                button { class: if !terminal() { "active" }, onclick: move |_| terminal.set(false), "Serial output" }
                button { class: if terminal() { "active" }, disabled: !enabled, onclick: move |_| terminal.set(true), "Terminal" }
            }
            if auth.is_admin() {
                button {
                    disabled: !editable || saving(),
                    onclick: move |_| {
                        let id = save_id.clone();
                        let mut spec = vm["spec"].clone();
                        spec["terminal"] = Value::Bool(!enabled);
                        saving.set(true);
                        spawn(async move {
                            match request("PUT", &format!("/v1/vms/{id}"), Some(spec), &auth.csrf()).await {
                                Ok(_) => { error.set(String::new()); terminal.set(false); onchanged.call(()); }
                                Err(value) => error.set(value),
                            }
                            saving.set(false);
                        });
                    },
                    if enabled { "Disable terminal" } else { "Enable terminal" }
                }
            }
        }
        if !editable && auth.is_admin() {
            p { class: "muted small", "Stop the VM to change terminal access." }
        }
        Notice { message: error() }
        if terminal() && enabled {
            terminal::TerminalPane { key: "terminal-{id}", id: id.clone() }
        } else {
            logs::LogOutput { key: "serial-{id}", id, stream: "serial" }
        }
    }
}

#[component]
pub fn FirecrackerLogs(id: String) -> Element {
    rsx! { logs::LogOutput { key: "firecracker-{id}", id, stream: "firecracker" } }
}
