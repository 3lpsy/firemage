use dioxus::prelude::*;
use firemage_webui_component_controls::{Field, Modal, Notice};
use firemage_webui_provider_api::{encode, get, text, upload_blob};
use firemage_webui_provider_auth::use_auth;
use wasm_bindgen::JsCast;

#[component]
pub fn Upload(onclose: EventHandler<()>, onsaved: EventHandler<String>) -> Element {
    let auth = use_auth();
    let limits = use_resource(|| async { get("/v1/snapshots/limits").await });
    let maximum = limits
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|value| value["max_bytes"].as_u64());
    let alias = use_signal(String::new);
    let mut file = use_signal(|| None::<web_sys::File>);
    let mut trusted = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    rsx! {
        Modal { title: "Upload snapshot", onclose: move |_| { if !busy() { onclose.call(()); } },
            form { onsubmit: move |event| {
                event.prevent_default();
                if busy() || !auth.is_admin() { return; }
                let Some(file) = file() else { return; };
                let Some(maximum) = maximum else { return; };
                let alias = alias();
                if let Err(message) = firemage_wire::ensure_asset_alias(&alias) { error.set(message.to_string()); return; }
                if file.size() > maximum as f64 { error.set("Snapshot exceeds the server upload limit.".into()); return; }
                busy.set(true); error.set(String::new());
                let path = format!("/v1/snapshots?alias={}&trusted={}", encode(&alias), trusted());
                spawn(async move {
                    match upload_blob(&path, file.unchecked_ref::<web_sys::Blob>(), &auth.csrf()).await {
                        Ok(snapshot) => onsaved.call(text(&snapshot, "id")), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                Field { id: "snapshot-upload-alias", label: "Alias", value: alias, required: true, disabled: busy(), placeholder: "imported-checkpoint" }
                label { class: "field", span { "Snapshot bundle" }
                    input { id: "snapshot-upload-file", r#type: "file", required: true, disabled: busy() || maximum.is_none(),
                        onchange: move |_| {
                            error.set(String::new()); file.set(None); trusted.set(false);
                            let picked = web_sys::window().and_then(|window| window.document()).and_then(|document| document.get_element_by_id("snapshot-upload-file"))
                                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok()).and_then(|input| input.files()).and_then(|files| files.get(0));
                            if let (Some(picked), Some(maximum)) = (picked, maximum) {
                                if picked.size() > maximum as f64 { error.set(format!("Snapshot uploads are limited to {}.", crate::model::size(maximum))); }
                                else { file.set(Some(picked)); }
                            }
                        }
                    }
                }
                if let Some(maximum) = maximum { p { class: "muted small", "Maximum bundle size: {crate::model::size(maximum)}." } }
                if let Some(Err(message)) = limits.read().as_ref() { Notice { message: message.clone() } }
                p { class: "muted small", "Bundles include guest memory and disks, which may contain credentials." }
                label { class: "snapshot-trust",
                    input { r#type: "checkbox", checked: trusted(), disabled: busy(), onchange: move |event| trusted.set(event.checked()) }
                    "I trust the source of this snapshot."
                }
                p { class: "muted small", "You can upload without trusting it. Trust is required before restore." }
                Notice { message: error() }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy() || file().is_none() || maximum.is_none() || !auth.is_admin(), if busy() { "Uploading…" } else { "Upload snapshot" } }
                }
            }
        }
    }
}
