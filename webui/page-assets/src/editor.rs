use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, request};
use firemage_webui_provider_auth::use_auth;
use firemage_wire::{FileAsset, ensure_asset_alias};
use serde_json::json;

#[component]
pub fn AliasEditor(
    asset: FileAsset,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
) -> Element {
    let auth = use_auth();
    let alias = use_signal(|| asset.alias.clone());
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    rsx! {
        Modal { title: "Edit asset alias", onclose: move |_| { if !busy() { onclose.call(()); } },
            form { onsubmit: move |event| {
                event.prevent_default(); if busy() { return; }
                let value = alias();
                if let Err(message) = ensure_asset_alias(&value) { error.set(message.to_string()); return; }
                let path = format!("/v1/assets/{}", encode(&asset.id));
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request("PUT", &path, Some(json!({"alias":value})), &auth.csrf()).await {
                        Ok(_) => onsaved.call(()), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                Notice { message: error() }
                Field { label: "Alias", id: "asset-edit-alias", value: alias, required: true, disabled: busy() }
                p { class: "small muted", "Alias must be unique in your library. Existing VM attachments stay unchanged." }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(), if busy() { "Saving…" } else { "Save alias" } }
                }
            }
        }
    }
}
