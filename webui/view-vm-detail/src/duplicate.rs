use dioxus::prelude::*;
use firemage_webui_component_controls::{Field, Modal, Notice};
use firemage_webui_provider_api::{encode, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn DuplicateVm(vm: Value, onclose: EventHandler<()>, onsaved: EventHandler<String>) -> Element {
    let auth = use_auth();
    let name = use_signal(|| {
        format!(
            "{}-copy",
            text(&vm["spec"], "name")
                .chars()
                .take(59)
                .collect::<String>()
        )
    });
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    rsx! {
        Modal { title: "Duplicate VM", onclose: move |_| { if !busy() { onclose.call(()); } },
            form { class: "modal-form", onsubmit: move |event| {
                event.prevent_default();
                if busy() { return; }
                let path = format!("/v1/vms/{}/duplicate", encode(&text(&vm, "id")));
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request("POST", &path, Some(json!({"name":name()})), &auth.csrf()).await {
                        Ok(value) => onsaved.call(text(&value, "id")), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    Field { label: "New VM name", id: "vm-duplicate-name", value: name, required: true, disabled: busy() }
                    p { class: "small muted", "Copies configuration only. Assets and secrets remain references; a networked VM gets new IP and MAC addresses. Start the new VM separately." }
                }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(), if busy() { "Duplicating…" } else { "Duplicate VM" } }
                }
            }
        }
    }
}
