mod import;
pub use import::ImportVm;

use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Configuration(vm: Value, onchanged: EventHandler<()>) -> Element {
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
    let toml = exported
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|value| text(value, "toml"));
    rsx! {
        div { class: "heading compact vm-tab-heading",
            h3 { "Configuration" }
            div { class: "actions wrap",
                if auth.is_admin() {
                    button { disabled: !editable, onclick: move |_| importing.set(true), Icon { name: "import", size: 16 } "Import" }
                }
                if let Some(toml) = &toml {
                    a { class: "button", href: format!("data:application/toml;charset=utf-8,{}", encode(toml)), download: "vm-config.toml", Icon { name: "export", size: 16 } "Export" }
                }
                button { class: "icon-button", "aria-label": "Refresh export", title: "Refresh export", onclick: move |_| exported.restart(), Icon { name: "refresh", size: 16 } }
            }
        }
        match exported.read().as_ref() {
            Some(Ok(value)) => rsx! {
                div { class: "actions end", CopyButton { source: CopySource::Text(text(value, "toml")), label: "Copy VM configuration" } }
                pre { class: "config-export", "{text(value, \"toml\")}" }
                p { class: "small muted", "Catalog references use aliases. Stored secrets remain name references. Configured literal values and userdata are included." }
            },
            Some(Err(error)) => rsx! { Notice { message: error.clone() } },
            None => rsx! { p { "Preparing export…" } },
        }
        if !editable { p { class: "small muted", "Stop the VM before importing or changing configuration." } }
        if importing() { ImportVm { existing_id: id, onclose: move |_| importing.set(false), onsaved: move |_| { importing.set(false); exported.restart(); onchanged.call(()); } } }
    }
}
