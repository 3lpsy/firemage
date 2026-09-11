use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, request, text, upload};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn AddKernel(onclose: EventHandler<()>, onsaved: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut source = use_signal(|| "upload".to_owned());
    let mut name = use_signal(String::new);
    let alias = use_signal(String::new);
    let mut url = use_signal(String::new);
    let sha = use_signal(String::new);
    let mut bytes = use_signal(|| None::<Vec<u8>>);
    let mut reading = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    rsx! {
        Modal { title: "Add kernel", onclose: move |_| { if !busy() && !reading() { onclose.call(()); } },
            form { class: "modal-form", onsubmit: move |event| {
                event.prevent_default();
                if busy() || reading() { return; }
                let body = if source() == "upload" { bytes.read().clone() } else { None };
                if source() == "upload" && body.is_none() { error.set("Choose a kernel file to upload.".into()); return; }
                busy.set(true); error.set(String::new());
                spawn(async move {
                    let result = if let Some(body) = body {
                        upload(&format!("/v1/kernels/{}/content?alias={}", encode(&name()), encode(&alias())), &body, &auth.csrf()).await
                    } else {
                        request("POST", "/v1/kernels/import", Some(json!({"name":name(), "url":url(), "sha256":sha(), "alias":alias()})), &auth.csrf()).await
                    };
                    match result { Ok(_) => onsaved.call(()), Err(message) => error.set(message) }
                    busy.set(false);
                });
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    fieldset { disabled: busy() || reading(),
                        legend { "Source" }
                        div { class: "radio-group",
                            for (value, label) in [("upload", "Upload file"), ("remote", "Remote URL")] {
                                label { input { r#type: "radio", name: "kernel-source", checked: source() == value, onchange: move |_| { source.set(value.into()); bytes.set(None); error.set(String::new()); } } "{label}" }
                            }
                        }
                    }
                    if source() == "upload" {
                        label { class: "field", r#for: "kernel-upload", span { "Kernel file" }
                            input { id: "kernel-upload", r#type: "file", disabled: busy() || reading(), onchange: move |event| {
                                bytes.set(None); error.set(String::new());
                                let Some(file) = event.files().into_iter().next() else { return; };
                                if file.size() > 128 * 1024 * 1024 { error.set("Kernel uploads are limited to 128 MiB.".into()); return; }
                                if name().is_empty() { name.set(file.name()); }
                                reading.set(true);
                                spawn(async move {
                                    match file.read_bytes().await { Ok(value) => bytes.set(Some(value.to_vec())), Err(message) => error.set(format!("Could not read kernel: {message}")) }
                                    reading.set(false);
                                });
                            } }
                        }
                    } else {
                        div { class: "field",
                            div { class: "secret-field-label", label { r#for: "kernel-url", "Kernel HTTPS URL" }
                                Info { title: "Kernel HTTPS URL", "The server downloads this public HTTPS URL once and stores the kernel in its library. Future VM starts use the saved file." }
                            }
                            input { id: "kernel-url", value: url(), required: true, disabled: busy(), placeholder: "https://example.com/vmlinux", oninput: move |event| url.set(event.value()) }
                        }
                        Field { label: "SHA-256 (optional)", id: "kernel-sha", value: sha, disabled: busy(), placeholder: "Optional checksum to verify the download" }
                    }
                    Field { label: "Filename in kernel directory", id: "kernel-name", value: name, required: true, disabled: busy() || reading(), placeholder: "vmlinux-6.1" }
                    AliasField { id: "kernel-add-alias", value: alias, disabled: busy() || reading(), placeholder: "linux-6.1" }
                    p { class: "small muted", "Maximum size: 128 MiB." }
                }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy() || reading(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy() || reading(), if busy() { "Adding kernel…" } else if reading() { "Reading file…" } else { "Add kernel" } }
                }
            }
        }
    }
}

#[component]
pub fn AliasEditor(kernel: Value, onclose: EventHandler<()>, onsaved: EventHandler<()>) -> Element {
    let auth = use_auth();
    let alias = use_signal(|| text(&kernel, "alias"));
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    rsx! {
        Modal { title: "Edit kernel alias", onclose,
            form { onsubmit: move |event| {
                event.prevent_default(); if busy() { return; }
                let path = format!("/v1/kernels/{}", encode(&text(&kernel, "name")));
                let value = alias().trim().to_owned();
                let body = json!({"alias":value});
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request("PUT", &path, Some(body), &auth.csrf()).await { Ok(_) => onsaved.call(()), Err(message) => error.set(message) }
                    busy.set(false);
                });
            },
                Notice { message: error() }
                AliasField { id: "kernel-alias", value: alias, disabled: busy(), placeholder: "Recommended" }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(), "Save alias" }
                }
            }
        }
    }
}

#[component]
fn AliasField(
    id: String,
    mut value: Signal<String>,
    disabled: bool,
    placeholder: String,
) -> Element {
    rsx! {
        div { class: "field",
            div { class: "secret-field-label", label { r#for: id.clone(), "Alias" }
                Info { title: "Kernel alias", "Unique kernel name used in VM configurations. Renaming preserves existing VM selections." }
            }
            input { id, value: value(), required: true, maxlength: 128, disabled, placeholder, oninput: move |event| value.set(event.value()) }
        }
    }
}
