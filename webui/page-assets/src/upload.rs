use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, request, upload_post};
use firemage_webui_provider_auth::use_auth;
use firemage_wire::{FileAssetLimits, FileAssetUpload};
use serde_json::json;

#[component]
pub fn AddAsset(onclose: EventHandler<()>, onsaved: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut limits = use_resource(|| async {
        serde_json::from_value::<FileAssetLimits>(get("/v1/assets/limits").await?)
            .map_err(|error| format!("Could not read asset size limit: {error}"))
    });
    let maximum = limits
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|limits| limits.max_bytes);
    let limit_label = maximum
        .map(crate::size_label)
        .unwrap_or_else(|| "Loading…".into());
    let mut source = use_signal(|| "upload");
    let alias = use_signal(String::new);
    let mut filename = use_signal(String::new);
    let remote_filename = use_signal(String::new);
    let mut url = use_signal(String::new);
    let sha = use_signal(String::new);
    let mut bytes = use_signal(|| None::<std::rc::Rc<Vec<u8>>>);
    let mut reading = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    rsx! {
        Modal { title: "Add asset", onclose: move |_| { if !busy() && !reading() { onclose.call(()); } },
            form { id: "asset-upload-section", class: "modal-form", onsubmit: move |event| {
                event.prevent_default();
                if busy() || reading() || maximum.is_none() { return; }
                let is_remote = source() == "remote";
                let input = FileAssetUpload { alias: alias(), filename: if is_remote { remote_filename() } else { filename() } };
                if let Err(message) = input.validate() { error.set(message.to_string()); return; }
                let body = if is_remote { None } else { bytes.read().clone() };
                if !is_remote && body.is_none() { error.set("Choose a file to upload.".into()); return; }
                let download = json!({
                    "alias": input.alias,
                    "filename": input.filename,
                    "url": url(),
                    "sha256": if sha().trim().is_empty() { None } else { Some(sha().trim().to_owned()) },
                });
                let path = format!("/v1/assets?alias={}&filename={}", encode(&input.alias), encode(&input.filename));
                busy.set(true); error.set(String::new());
                spawn(async move {
                    let result = if let Some(body) = body {
                        upload_post(&path, &body, &auth.csrf()).await
                    } else {
                        request("POST", "/v1/assets/import", Some(download), &auth.csrf()).await
                    };
                    match result { Ok(_) => onsaved.call(()), Err(message) => error.set(message) }
                    busy.set(false);
                });
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    if let Some(Err(message)) = limits.read().as_ref() {
                        Notice { message: message.clone() }
                        button { r#type: "button", onclick: move |_| limits.restart(), "Retry asset limit" }
                    }
                    fieldset { disabled: busy() || reading(),
                        legend { "Source" }
                        div { class: "radio-group",
                            for (value, label) in [("upload", "Upload file"), ("remote", "Remote URL")] {
                                label {
                                    input { r#type: "radio", name: "asset-source", checked: source() == value,
                                        onchange: move |_| { source.set(value); error.set(String::new()); bytes.set(None); filename.set(String::new()); }
                                    }
                                    "{label}"
                                }
                            }
                        }
                    }
                    if source() == "upload" {
                        label { class: "field", r#for: "asset-file", span { "File" }
                            input { id: "asset-file", r#type: "file", required: true, disabled: busy() || reading() || maximum.is_none(), onchange: move |event| {
                                bytes.set(None); filename.set(String::new()); error.set(String::new());
                                let Some(file) = event.files().into_iter().next() else { return; };
                                let Some(maximum) = maximum else { return; };
                                if file.size() > maximum { error.set(format!("Asset uploads are limited to {}.", crate::size_label(maximum))); return; }
                                filename.set(file.name()); reading.set(true);
                                spawn(async move {
                                    match file.read_bytes().await {
                                        Ok(value) => bytes.set(Some(std::rc::Rc::new(value.to_vec()))),
                                        Err(message) => error.set(format!("Could not read file: {message}")),
                                    }
                                    reading.set(false);
                                });
                            } }
                        }
                    } else {
                        div { class: "field",
                            div { class: "secret-field-label", label { r#for: "asset-import-url", "Asset HTTPS URL" }
                                Info { title: "Asset HTTPS URL", "The server downloads this public HTTPS URL once and saves the file in your asset library. Add a SHA-256 checksum to verify its contents." }
                            }
                            input { id: "asset-import-url", value: url(), required: true, disabled: busy(), placeholder: "https://example.com/config.toml", oninput: move |event| url.set(event.value()) }
                        }
                        Field { label: "Filename", id: "asset-import-filename", value: remote_filename, required: true, disabled: busy(), placeholder: "config.toml" }
                        Field { label: "SHA-256 (optional)", id: "asset-import-sha", value: sha, disabled: busy() }
                    }
                    crate::editor::AliasField { id: "asset-alias", value: alias, disabled: busy() || reading() }
                    p { class: "small muted", "Maximum file size: {limit_label}." }
                }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy() || reading(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy() || reading() || maximum.is_none() || (source() == "upload" && bytes.read().is_none()),
                        if busy() { if source() == "remote" { "Downloading…" } else { "Uploading…" } } else if reading() { "Reading file…" } else { "Add asset" }
                    }
                }
            }
        }
    }
}
