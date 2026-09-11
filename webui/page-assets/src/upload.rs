use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, upload_post};
use firemage_webui_provider_auth::use_auth;
use firemage_wire::{FileAssetLimits, FileAssetUpload};

#[component]
pub fn UploadAsset(
    onsaved: EventHandler<()>,
    onmounted: EventHandler<std::rc::Rc<MountedData>>,
) -> Element {
    let auth = use_auth();
    let mut limits = use_resource(|| async {
        serde_json::from_value::<FileAssetLimits>(get("/v1/assets/limits").await?)
            .map_err(|error| format!("Could not read asset upload limit: {error}"))
    });
    let maximum = limits
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|limits| limits.max_bytes);
    let limit_label = maximum
        .map(crate::size_label)
        .unwrap_or_else(|| "Loading…".into());
    let mut alias = use_signal(String::new);
    let mut filename = use_signal(String::new);
    let mut bytes = use_signal(|| None::<std::rc::Rc<Vec<u8>>>);
    let mut reading = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut generation = use_signal(|| 0u64);
    rsx! {
        section { id: "asset-upload-section", "aria-labelledby": "asset-upload-title",
            h2 { id: "asset-upload-title", "Upload asset" }
            form { onsubmit: move |event| {
                event.prevent_default();
                if busy() || reading() { return; }
                status.set(String::new());
                let input = FileAssetUpload { alias: alias(), filename: filename() };
                if let Err(message) = input.validate() { error.set(message.to_string()); return; }
                let Some(body) = bytes.read().clone() else { error.set("Choose a file to upload.".into()); return; };
                let path = format!("/v1/assets?alias={}&filename={}", encode(&input.alias), encode(&input.filename));
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match upload_post(&path, &body, &auth.csrf()).await {
                        Ok(_) => {
                            status.set(format!("Uploaded {}.", input.alias));
                            alias.set(String::new()); filename.set(String::new()); bytes.set(None);
                            generation += 1;
                            onsaved.call(());
                        },
                        Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                Notice { message: error() }
                if let Some(Err(message)) = limits.read().as_ref() {
                    Notice { message: message.clone() }
                    button { r#type: "button", onclick: move |_| limits.restart(), "Retry upload limit" }
                }
                div { class: "form-grid",
                    label { class: "field", r#for: "asset-alias", span { "Alias" }
                        input { id: "asset-alias", value: "{alias}", required: true, maxlength: 128, disabled: busy() || reading(), placeholder: "app-config", oninput: move |event| alias.set(event.value()), onmounted: move |event| onmounted.call(event.data()) }
                    }
                    label { class: "field", r#for: "asset-file", span { "File" }
                        // A keyed dynamic child replaces the native file input when the form resets.
                        for input_generation in [generation()] {
                        input { key: "{input_generation}", id: "asset-file", r#type: "file", required: true, disabled: busy() || reading() || maximum.is_none(), onchange: move |event| {
                            bytes.set(None); filename.set(String::new()); error.set(String::new()); status.set(String::new());
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
                    }
                }
                p { class: "small muted", "Alias must be unique in your library. Maximum file size: {limit_label}." }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy() || reading(), onclick: move |_| {
                        alias.set(String::new()); filename.set(String::new()); bytes.set(None);
                        error.set(String::new()); status.set(String::new()); generation += 1;
                    }, "Clear" }
                    button { r#type: "submit", class: "primary", disabled: busy() || reading() || bytes.read().is_none(),
                        if busy() { "Uploading…" } else if reading() { "Reading file…" } else { "Upload" }
                    }
                }
                if !status().is_empty() { p { role: "status", "{status}" } }
            }
        }
    }
}
