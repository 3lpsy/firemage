use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn ImportVm(
    #[props(default)] existing_id: String,
    onclose: EventHandler<()>,
    onsaved: EventHandler<String>,
) -> Element {
    let auth = use_auth();
    let mut source = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut preview = use_signal(|| None::<Value>);
    let existing = !existing_id.is_empty();
    let preview_path = if existing {
        format!("/v1/vms/{existing_id}/config/preview")
    } else {
        "/v1/vm-config/preview".into()
    };
    let import_path = if existing {
        format!("/v1/vms/{existing_id}/config")
    } else {
        "/v1/vm-config/import".into()
    };
    rsx! {
        Modal { title: if existing { "Import configuration" } else { "Import VM" }, onclose: move |_| { if !busy() { onclose.call(()); } },
            div { class: "modal-form",
                div { class: "modal-form-body",
                    Notice { message: error() }
                    label { class: "field", r#for: "vm-import-file", span { "Config file" }
                        input { id: "vm-import-file", r#type: "file", accept: ".toml", disabled: busy(), onchange: move |event| {
                            let Some(file) = event.files().into_iter().next() else { return; };
                            if file.size() > 1024 * 1024 { error.set("Config files must be at most 1 MiB.".into()); return; }
                            preview.set(None); error.set(String::new()); busy.set(true);
                            spawn(async move {
                                match file.read_bytes().await {
                                    Ok(bytes) => match String::from_utf8(bytes.to_vec()) { Ok(value) => source.set(value), Err(_) => error.set("Config must contain UTF-8 TOML.".into()) },
                                    Err(message) => error.set(message.to_string()),
                                }
                                busy.set(false);
                            });
                        } }
                    }
                    label { class: "field", r#for: "vm-import-name", span { "VM name override (optional)" }
                        input { id: "vm-import-name", value: name(), maxlength: 64, disabled: busy(), placeholder: "Use the name from the config", oninput: move |event| { name.set(event.value()); preview.set(None); } }
                    }
                    label { class: "field", r#for: "vm-import-toml", span { "Configuration TOML" }
                        textarea { id: "vm-import-toml", rows: 14, value: source(), disabled: busy(), oninput: move |event| { source.set(event.value()); preview.set(None); } }
                    }
                    p { class: "small muted", "References must match aliases and secret names on this server. Edit references above to map them. Secret values stay in Secrets; imports reference existing names." }
                    if let Some(value) = preview() {
                        h3 { "Resolved references" }
                        table { thead { tr { th { "Type" } th { "Alias / name" } } }
                            tbody { for item in value["references"].as_array().into_iter().flatten() { tr { td { "{text(item, \"kind\")}" } td { "{text(item, \"alias\")}" } } } }
                        }
                        p { "VM: {text(&value, \"name\")}" }
                        if value["address"].is_string() { p { class: "small", "Guest IP: {text(&value, \"address\")}" } }
                        p { class: "small muted", if existing { "The existing VM definition will be updated. Existing disk replacement is not allowed." } else { "Import saves the definition without starting it. New IP and MAC addresses are suggested from the selected network." } }
                    }
                }
                div { class: "modal-form-footer actions end",
                    button { disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { disabled: busy() || source().trim().is_empty(), onclick: move |_| {
                        let path = preview_path.clone();
                        let body = json!({"toml":source(),"name":if name().is_empty() {None} else {Some(name())}});
                        busy.set(true); error.set(String::new()); preview.set(None);
                        spawn(async move { match request("POST", &path, Some(body), &auth.csrf()).await { Ok(value) => preview.set(Some(value)), Err(message) => error.set(message) } busy.set(false); });
                    }, "Validate references" }
                    button { class: "primary", disabled: busy() || preview().is_none(), onclick: move |_| {
                        let path = import_path.clone();
                        let body = json!({"toml":source(),"name":if name().is_empty() {None} else {Some(name())}});
                        busy.set(true); error.set(String::new());
                        spawn(async move { match request(if existing {"PUT"} else {"POST"}, &path, Some(body), &auth.csrf()).await { Ok(value) => onsaved.call(text(&value,"id")), Err(message) => {error.set(message); preview.set(None);} } busy.set(false); });
                    }, if existing { "Import config" } else { "Import VM" } }
                }
            }
        }
    }
}
