//! Named network policy inventory and editor.
mod editor;
use dioxus::prelude::*;
use editor::NetworkEditor;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;
#[component]
pub fn Networks() -> Element {
    let auth = use_auth();
    let mut refresh = use_signal(|| 0);
    let mut editor = use_signal(|| None::<Value>);
    let mut deleting = use_signal(String::new);
    let mut error = use_signal(String::new);
    let rows = use_resource(move || {
        let _ = refresh();
        async { get("/v1/networks").await }
    });
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "CONNECTIVITY" }
                h1 { "Networks" }
                p { class: "muted", "Explicit access policies for every guest." }
            }
            if auth.is_admin() {
                button {
                    class: "primary",
                    onclick: move | _ | editor
                                                .set(Some(Value::Null)),
                    "+ Create network"
                }
            }
        }
        Notice { message: error() }
        div { class: "policy-guide",
            span { class: "purple",
                Icon { name: "networks" }
            }
            p {
                "Offline is a first-class configuration. VMs have no interface until you attach a network."
            }
            Info { title: "Network policies",
                "Firemage only routes guest requests through that VM’s HTTP proxy rules and TCP tunnels. Isolated blocks traffic through the host. Specific host IP permits one host IP address. Unrestricted enables routed access. A VM with no network attachment has no network device."
            }
        }
        match rows.read().as_ref() {
            Some(Ok(value)) => rsx! {
                if value.as_array().is_none_or(Vec::is_empty) {
                    Empty {
                        title: "No networks defined",
                        description: "Create a network when a VM needs an interface or controlled host access.",
                    }
                } else {
                    table {
                        thead {
                            tr {
                                th { "NAME" }
                                th { "SUBNET" }
                                th { "GATEWAY" }
                                th { "POLICY" }
                                th { "" }
                            }
                        }
                        tbody {
                            for row in value.as_array().into_iter().flatten() {
                                tr {
                                    td {
                                        strong { r#"{row["name"].as_str().unwrap_or_default()}"# }
                                    }
                                    td { class: "mono", r#"{row["subnet"].as_str().unwrap_or_default()}"# }
                                    td { class: "mono", r#"{row["gateway"].as_str().unwrap_or_default()}"# }
                                    td {
                                        Status { value: text(&row["policy"], "mode") }
                                        if row["policy"]["address"].is_string() {
                                            small { class: "mono",
                                                r#"{row["policy"]["address"].as_str().unwrap_or_default()}"#
                                            }
                                        }
                                    }
                                    td {
                                        if auth.is_admin() {
                                            div { class: "actions end",
                                                button {
                                                    onclick: {
                                                        let row = row.clone();
                                                        move |_| editor.set(Some(row.clone()))
                                                    },
                                                    "Edit"
                                                }
                                                button {
                                                    class: "danger subtle",
                                                    onclick: {
                                                        let name = text(row, "name");
                                                        move |_| deleting.set(name.clone())
                                                    },
                                                    "Delete"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! {
                Notice { message: e.clone() }
            },
            None => rsx! {
                p { class: "muted", "Loading networks…" }
            },
        }
        if let Some(initial) = editor() {
            NetworkEditor {
                initial,
                onclose: move |_| editor.set(None),
                onsaved: move |_| {
                    editor.set(None);
                    refresh += 1;
                },
            }
        }
        if !deleting().is_empty() {
            Confirm {
                title: "Delete network?",
                description: format!("Remove {}? Detach it from all VMs first.", deleting()),
                label: "Delete network",
                onclose: move |_| deleting.set(String::new()),
                onconfirm: move |_| {
                    let name = deleting();
                    deleting.set(String::new());
                    spawn(async move {
                        match request(
                                "DELETE",
                                &format!("/v1/networks/{}", encode(&name)),
                                None,
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(_) => refresh += 1,
                            Err(e) => error.set(e),
                        }
                    });
                },
            }
        }
    }
}
