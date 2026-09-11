mod import;
pub use import::ImportVm;

use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Configuration(vm: Value, onedit: EventHandler<()>, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut importing = use_signal(|| false);
    let id = text(&vm, "id");
    let editable = matches!(
        text(&vm, "state").as_str(),
        "defined" | "stopped" | "failed"
    );
    let mut exported = use_resource(use_reactive((&vm,), |(vm,)| {
        let id = text(&vm, "id");
        async move { get(&format!("/v1/vms/{id}/config")).await }
    }));
    rsx! {
        div { class: "heading compact",
            h3 { "Configuration" }
            div { class: "actions wrap",
                if auth.is_admin() {
                    button { disabled: !editable, onclick: move |_| onedit.call(()), "Configure VM" }
                    button { disabled: !editable, onclick: move |_| importing.set(true), "Import config" }
                }
                button { onclick: move |_| exported.restart(), "Refresh export" }
            }
        }
        p { class: "small muted", "Catalog references use aliases. Stored secrets remain name references. Configured literal values and userdata are included." }
        match exported.read().as_ref() {
            Some(Ok(value)) => rsx! {
                a { class: "button", href: format!("data:application/toml;charset=utf-8,{}", encode(value["toml"].as_str().unwrap_or_default())), download: "vm-config.toml", "Export config" }
                pre { class: "config-export", "{text(value, \"toml\")}" }
            },
            Some(Err(error)) => rsx! { Notice { message: error.clone() } },
            None => rsx! { p { "Preparing export…" } },
        }
        if !editable { p { class: "small muted", "Stop the VM before importing or changing configuration." } }
        if importing() { ImportVm { existing_id: id, onclose: move |_| importing.set(false), onsaved: move |_| { importing.set(false); exported.restart(); onchanged.call(()); } } }
    }
}
