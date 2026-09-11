use dioxus::prelude::*;
use firemage_webui_component_controls::{Editor, Field, Info, Modal, Notice, SecretFieldLabel};
use firemage_webui_component_value_source::ValueSource;
use firemage_webui_provider_api::{get, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn ProxyEditor(
    #[props(default)] value: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<Value>,
) -> Element {
    let auth = use_auth();
    let alias = use_signal(|| text(&value, "alias"));
    let url = use_signal(|| text(&value["proxy"], "url"));
    let mut username = use_signal(|| value["proxy"]["username"].clone());
    let mut password = use_signal(|| text(&value["proxy"]["password"], "secret"));
    let mut ca = use_signal(|| text(&value, "ca_secret"));
    let pem = use_signal(|| text(&value["proxy"], "ca_pem"));
    let mut secrets = use_resource(|| async { get("/v1/secrets").await });
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let existing = value["id"].is_string();
    let live = crate::catalog_model::running(&value);
    let count = crate::catalog_model::usage(&value, "vm");
    let policies = crate::catalog_model::usage(&value, "policy");
    rsx! {
        Modal { title: if existing { "Edit upstream proxy" } else { "Create upstream proxy" }, onclose: move |_| { if !busy() { onclose.call(()); } },
            form { class: "modal-form", onsubmit: move |event| {
                event.prevent_default(); if busy() { return; }
                let mut proxy = json!({"url":url().trim()});
                for (name, source) in [("username", username()), ("password", if password().is_empty() { Value::Null } else { json!({"secret":password()}) })] {
                    if !source.is_null() && source.as_str() != Some("") { proxy[name] = source; }
                }
                if ca().is_empty() && !pem().trim().is_empty() { proxy["ca_pem"] = json!(pem()); }
                if alias().trim().is_empty() { error.set("Enter a unique proxy alias.".into()); return; }
                let parsed = serde_json::from_value::<firemage_wire::UpstreamProxy>(proxy.clone()).map_err(|e|e.to_string()).and_then(|p|p.validate().map_err(|e|e.to_string()));
                if let Err(message) = parsed { error.set(message); return; }
                let mut body = json!({"alias":alias().trim(),"proxy":proxy,"ca_secret":if ca().is_empty() { Value::Null } else { json!(ca()) }});
                let path = if existing { body["revision"] = value["revision"].clone(); format!("/v1/egress/proxies/{}",text(&value,"id")) } else { "/v1/egress/proxies".into() };
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request(if existing { "PUT" } else { "POST" },&path,Some(body),&auth.csrf()).await {
                        Ok(saved) => onsaved.call(saved), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    fieldset { class: "vm-editor-body", disabled: busy(),
                        Field { label: "Alias", id: "proxy-alias", value: alias, required: true }
                        Field { label: "Proxy URL", id: "proxy-url", value: url, required: true, placeholder: "https://proxy.example.com:3129" }
                        p { class: "small muted", "Supports http://, https:// and socks5://. Keep credentials out of the URL." }
                        ValueSource { label: "Username (optional)", id: "proxy-username", value: username(), plain_text: true, onchange: move |value| username.set(value) }
                        div { class: "field",
                            SecretFieldLabel { id: "proxy-password-secret", label: "Password secret (optional)", onrefresh: move |_| secrets.restart() }
                            select { id: "proxy-password-secret", disabled: !secrets.read().as_ref().is_some_and(Result::is_ok), value: password(), onchange: move |event| password.set(event.value()),
                                option { value: "", selected: password().is_empty(), "No password" }
                                if let Some(Ok(rows)) = secrets.read().as_ref() {
                                    for row in rows.as_array().into_iter().flatten() { option { value: text(row,"name"), selected: text(row,"name") == password(), "{text(row,\"name\")}" } }
                                    if !password().is_empty() && !rows.as_array().into_iter().flatten().any(|row| text(row,"name") == password()) { option { value: password(), "{password} (unavailable)" } }
                                }
                            }
                            p { class: "small muted", "Set both username and password to authenticate, or leave both empty." }
                        }
                        div { class: "field",
                            SecretFieldLabel { id: "proxy-ca-secret", label: "Custom CA secret (optional)", onrefresh: move |_| secrets.restart(),
                                help: rsx! { Info { title: "Custom proxy CA", "For HTTPS proxies using a private CA. The secret contains PEM certificates and stays on the server." } }
                            }
                            select { id: "proxy-ca-secret", disabled: !secrets.read().as_ref().is_some_and(Result::is_ok), value: ca(), onchange: move |event| ca.set(event.value()),
                                option { value: "", selected: ca().is_empty(), if pem().is_empty() { "System trust" } else { "Existing CA certificates" } }
                                if let Some(Ok(rows)) = secrets.read().as_ref() {
                                    for row in rows.as_array().into_iter().flatten() { option { value: text(row,"name"), selected: text(row,"name") == ca(), "{text(row,\"name\")}" } }
                                    if !ca().is_empty() && !rows.as_array().into_iter().flatten().any(|row| text(row,"name") == ca()) { option { value: ca(), "{ca} (unavailable)" } }
                                }
                            }
                            if let Some(Err(message)) = secrets.read().as_ref() { Notice { message: message.clone() } }
                        }
                        if !pem().is_empty() { Editor { label: "Existing CA certificates (used when no CA secret is selected)", id: "proxy-ca-pem", value: pem, rows: 4 } }
                    }
                    if existing && count > 0 { p { class: "notice", "Updates {policies} policies and {count} VMs, including {live} running or paused. Existing egress connections close and guest programs reconnect." } }
                }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(), if busy() { "Applying…" } else if existing { "Save proxy" } else { "Create proxy" } }
                }
            }
        }
    }
}
