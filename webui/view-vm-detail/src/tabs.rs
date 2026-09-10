use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, pretty, request};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
#[component]
pub fn Logs(id: String) -> Element {
    let mut refresh = use_signal(|| 0u32);
    let log = use_resource(move || {
        let id = id.clone();
        let _ = refresh();
        async move { get(&format!("/v1/vms/{id}/logs")).await }
    });
    rsx! {
        div { class: "heading compact",
            h3 { "Serial output" }
            button { onclick: move |
                                        _ | refresh += 1, "Refresh logs" }
        }
        match log.read().as_ref() {
            Some(Ok(value)) => rsx! {
                pre { class: "console", r#"{value["text"].as_str().unwrap_or("No output yet.")}"# }
            },
            Some(Err(error)) => rsx! {
                Notice { message: error.clone() }
            },
            None => rsx! {
                p { class: "muted", "Loading logs…" }
            },
        }
    }
}
#[component]
pub fn Files(id: String) -> Element {
    let path = use_signal(|| "firemage/output/result".to_owned());
    let mut result = use_signal(|| Value::Null);
    let mut error = use_signal(String::new);
    rsx! {
        h3 {
            "Guest output"
            Info { title: "Guest files",
                "After a VM stops, read a file from its root disk using a relative path. Boot files are available to the guest on the seed disk. File downloads preserve binary bytes."
            }
        }
        form {
            onsubmit: move |e| {
                e.prevent_default();
                let id = id.clone();
                error.set(String::new());
                result.set(Value::Null);
                spawn(async move {
                    match get(&format!("/v1/vms/{id}/files?path={}", encode(&path()))).await {
                        Ok(value) => result.set(value),
                        Err(e) => error.set(e),
                    }
                });
            },
            Field {
                label: "Path in root disk",
                id: "output-path",
                value: path,
                required: true,
            }
            button { r#type: "submit", "Read file" }
        }
        Notice { message: error() }
        if result()["base64"].is_string() {
            a {
                class: "button primary",
                href: r#"data:application/octet-stream;base64,{result()["base64"].as_str().unwrap_or_default()}"#,
                download: path().rsplit('/').next().unwrap_or("output").to_owned(),
                "Download file"
            }
            p { class: "muted small", "File is ready. The download preserves its original encoding." }
        }
    }
}
#[component]
pub fn Metadata(id: String, initial: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let value = use_signal(|| pretty(&initial));
    let mut message = use_signal(String::new);
    let mut success = use_signal(|| false);
    rsx! {
        h3 {
            "Instance metadata"
            Info { title: "Metadata service",
                "Firecracker MMDS v2 serves JSON inside a guest with a network interface. Use an isolated network when the guest needs metadata but no external access. The guest must request an MMDS session token."
            }
        }
        Editor {
            label: "Metadata JSON",
            id: "vm-metadata",
            value,
            rows: 12,
            disabled: !auth.is_admin(),
        }
        Notice { message: message(), success: success() }
        if auth.is_admin() {
            button {
                class: "primary",
                onclick: move |_| {
                    let id = id.clone();
                    let body = match serde_json::from_str::<Value>(&value()) {
                        Ok(value) => value,
                        Err(e) => {
                            success.set(false);
                            message.set(e.to_string());
                            return;
                        }
                    };
                    spawn(async move {
                        match request(
                                "POST",
                                &format!("/v1/vms/{id}/actions"),
                                Some(json!({ "action" : "metadata", "value" : body })),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(_) => {
                                success.set(true);
                                message.set("Metadata updated".into());
                                onchanged.call(());
                            }
                            Err(e) => {
                                success.set(false);
                                message.set(e);
                            }
                        }
                    });
                },
                "Update metadata"
            }
        }
    }
}
