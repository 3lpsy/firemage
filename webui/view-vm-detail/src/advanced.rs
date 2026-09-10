use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{pretty, request};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
#[component]
pub fn Advanced(id: String, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut method = use_signal(|| "GET".to_owned());
    let path = use_signal(|| "/".to_owned());
    let body = use_signal(|| "{}".to_owned());
    let mut output = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut snapshot_action = use_signal(|| "snapshot".to_owned());
    let snapshot = use_signal(String::new);
    let memory = use_signal(String::new);
    let snapshot_id = id.clone();
    rsx! {
        h3 {
            "Firecracker API"
            Info { title: "Raw API access",
                "Send a request to this VM's Firecracker socket. Jailed VMs permit only operations that preserve managed isolation. Other writes can change runtime state outside the saved definition."
            }
        }
        Notice { message: error() }
        if auth.is_admin() {
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    let id = id.clone();
                    let body = if method() == "GET" {
                        None
                    } else {
                        match serde_json::from_str::<Value>(&body()) {
                            Ok(v) => Some(v),
                            Err(e) => {
                                error.set(e.to_string());
                                return;
                            }
                        }
                    };
                    error.set(String::new());
                    spawn(async move {
                        match request(
                                "POST",
                                &format!("/v1/vms/{id}/firecracker"),
                                Some(json!({ "method" : method(), "path" : path(), "body" : body })),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(v) => {
                                output.set(pretty(&v));
                                onchanged.call(());
                            }
                            Err(e) => error.set(e),
                        }
                    });
                },
                label { class: "field", r#for: "raw-method",
                    span { "Method" }
                    select {
                        id: "raw-method",
                        value: "{method}",
                        onchange: move |e| method.set(e.value()),
                        for value in ["GET", "PUT", "PATCH"] {
                            option { value, "{value}" }
                        }
                    }
                }
                Field {
                    label: "API path",
                    id: "raw-path",
                    value: path,
                    required: true,
                }
                if method() != "GET" {
                    Editor {
                        label: "Request JSON",
                        id: "raw-body",
                        value: body,
                        rows: 5,
                    }
                }
                button { r#type: "submit", "Send request" }
            }
            if !output().is_empty() {
                pre { class: "console", "{output}" }
            }
            hr {}
            h3 { "Snapshots"
                Info { title: "Snapshot storage",
                    "Pause the VM before saving a snapshot. Jailed VMs accept filenames in their private snapshot directory and can restore only their own recorded snapshots. Trusted or external VMs require absolute server paths."
                }
            }
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    let id = snapshot_id.clone();
                    error.set(String::new());
                    spawn(async move {
                        match request(
                                "POST",
                                &format!("/v1/vms/{id}/actions"),
                                Some(
                                    json!(
                                        { "action" : snapshot_action(), "snapshot_path" : snapshot(),
                                        "memory_path" : memory() }
                                    ),
                                ),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(v) => {
                                output.set(pretty(&v));
                                onchanged.call(());
                            }
                            Err(e) => error.set(e),
                        }
                    });
                },
                fieldset {
                    legend { "Operation" }
                    div { class: "radio-group",
                        for (value, label) in [("snapshot", "Save snapshot"), ("restore", "Restore snapshot")] {
                            label {
                                input {
                                    r#type: "radio",
                                    name: "snapshot-action",
                                    checked: snapshot_action() == value,
                                    onchange: move | _ |
                                                                                snapshot_action.set(value.into()),
                                }
                                "{label}"
                            }
                        }
                    }
                }
                Field {
                    label: "Snapshot filename or path",
                    id: "snapshot-path",
                    value: snapshot,
                    required: true,
                }
                Field {
                    label: "Memory filename or path",
                    id: "snapshot-memory",
                    value: memory,
                    required: true,
                }
                button { r#type: "submit", "Run snapshot operation" }
            }
        } else {
            p { class: "muted", "Administrator access is required." }
        }
    }
}
