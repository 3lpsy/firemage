//! Optional interactive guest shell and its VM configuration controls.
mod session;
mod settings;
use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use firemage_webui_provider_api::{get, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
pub use settings::Settings;

#[component]
pub fn WebShell(vm: Value, onchanged: EventHandler<()>) -> Element {
    let id = text(&vm, "id");
    rsx! {
        style { {include_str!("shell.css")} }
        for key in [id] {
            ShellView { key: "{key}", vm: vm.clone(), onchanged }
        }
    }
}

#[component]
fn ShellView(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let id = text(&vm, "id");
    let state = text(&vm, "state");
    let enabled = vm["spec"]["web_terminal"].is_object();
    let can_edit = matches!(state.as_str(), "defined" | "stopped" | "failed");
    rsx! {
        Notice { message: error() }
        if vm["spec"]["socket"].is_string() {
            p { class: "muted", "Web Terminal requires a VM managed by Firemage." }
        } else if !auth.is_admin() {
            p { class: "muted", "Administrator access is required for Web Shell." }
        } else if !enabled {
            h3 { "Web Shell" }
            p { "Enable Web Terminal to open an interactive guest shell." }
            p { class: "small muted", "Managed OCI images start the helper automatically. Custom images must start firemage-guest from their own init." }
            button { class: "primary", disabled: busy() || !can_edit,
                onclick: move |_| {
                    let id = id.clone();
                    busy.set(true); error.set(String::new());
                    spawn(async move {
                        let result = async {
                            let current = get(&format!("/v1/vms/{id}")).await?;
                            let mut spec = current["spec"].clone();
                            spec["web_terminal"] = json!({"command":["/bin/sh","-i"]});
                            request("PUT", &format!("/v1/vms/{id}"), Some(spec), &auth.csrf()).await
                        }.await;
                        match result { Ok(_) => onchanged.call(()), Err(message) => error.set(message) }
                        busy.set(false);
                    });
                }, "Enable Web Terminal"
            }
            if !can_edit { p { class: "small muted", "Stop the VM to enable Web Terminal." } }
        } else if state != "running" {
            h3 { "Web Shell" }
            p { class: "muted", "Start the VM to open a shell." }
        } else {
            session::Session { id, csrf: auth.csrf() }
        }
    }
}
