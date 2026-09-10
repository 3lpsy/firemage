use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, request};
use firemage_webui_provider_auth::use_auth;
use serde_json::json;

#[component]
pub fn SecretEditor(name: String, onclose: EventHandler<()>, onsaved: EventHandler<()>) -> Element {
    let auth = use_auth();
    let replacing = !name.is_empty();
    let name = use_signal(|| name);
    let mut value = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: if replacing { "Replace secret value" } else { "Create secret" }, onclose,
            form { onsubmit: move |event| {
                event.prevent_default();
                if busy() { return; }
                if name().is_empty() || name().len() > 64 || !name().bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)) {
                    error.set("Use 1–64 letters, digits, underscores, or hyphens for the name.".into()); return;
                }
                if value().is_empty() { error.set("A secret value is required.".into()); return; }
                busy.set(true); error.set(String::new());
                let path = format!("/v1/secrets/{}", encode(&name()));
                let body = json!({"value":value()});
                spawn(async move {
                    match request("PUT", &path, Some(body), &auth.csrf()).await {
                        Ok(_) => { value.set(String::new()); onsaved.call(()); },
                        Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                Notice { message: error() }
                Field { label: "Secret name", id: "secret-name", value: name, required: true, disabled: replacing, placeholder: "openai-key" }
                Field { label: "Secret value", id: "secret-value", value, kind: "password", required: true }
                p { class: "small muted", "The value is never returned by the API or shown after saving." }
                div { class: "actions end",
                    button { r#type: "button", onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(), "Save secret" }
                }
            }
        }
    }
}
