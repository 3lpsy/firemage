//! Directory-backed kernel inventory and management.
mod editor;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Kernels() -> Element {
    let auth = use_auth();
    let mut rows = use_resource(|| async { get("/v1/kernels").await });
    let mut search = use_signal(String::new);
    let mut adding = use_signal(|| false);
    let mut alias = use_signal(|| None::<Value>);
    let mut deleting = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "COMPUTE" }
                div { class: "actions",
                    h1 { "Kernels" }
                    Info { title: "Kernel library",
                        "This list reads the configured kernel directory, including files installed directly on the server. Uploads and downloads stay in that directory. Aliases are display names; VMs keep their selected filename. Remove VM references before deleting a kernel."
                    }
                }
            }
            div { class: "actions",
                button { class: "icon-button", title: "Refresh kernels", "aria-label": "Refresh kernels", onclick: move |_| rows.restart(), Icon { name: "refresh", size: 16 } }
                if auth.is_admin() {
                    button { class: "primary", onclick: move |_| adding.set(true), "+ Add kernel" }
                }
            }
        }
        Notice { message: error() }
        div { class: "toolbar",
            input { id: "kernel-search", r#type: "search", "aria-label": "Search kernels", placeholder: "Search filename or alias", value: "{search}", oninput: move |event| search.set(event.value()) }
        }
        match rows.read().as_ref() {
            Some(Ok(value)) => {
                let filtered: Vec<_> = value.as_array().into_iter().flatten().filter(|row| {
                    format!("{} {}", text(row, "name"), text(row, "alias")).to_lowercase().contains(&search().to_lowercase())
                }).collect();
                rsx! {
                    if filtered.is_empty() {
                        Empty { title: if search().is_empty() { "No kernels available" } else { "No matching kernels" }, description: "Add a kernel or refresh after placing one in the server's kernel directory." }
                    } else {
                        table { class: "kernel-table",
                            thead { tr { th { "KERNEL" } th { "ALIAS" } th { "SIZE" } th { "ACTIONS" } } }
                            tbody {
                                for row in filtered {
                                    tr { key: r#"{text(row, "name")}"#,
                                        td { class: "mono", r#"{text(row, "name")}"# }
                                        td { if text(row, "alias").is_empty() { "None" } else { r#"{text(row, "alias")}"# } }
                                        td { class: "mono", {format!("{:.1} MiB", row["size_bytes"].as_u64().unwrap_or(0) as f64 / 1_048_576.0)} }
                                        td {
                                            if auth.is_admin() {
                                                div { class: "actions end",
                                                    button { onclick: { let row = row.clone(); move |_| alias.set(Some(row.clone())) }, "Edit alias" }
                                                    button { class: "danger subtle", disabled: busy() || row["vm_count"].as_u64().unwrap_or(0) > 0, title: if row["vm_count"].as_u64().unwrap_or(0) > 0 { "Referenced by a VM" } else { "Remove kernel from disk" },
                                                        onclick: { let name = text(row, "name"); move |_| deleting.set(name.clone()) }, "Delete"
                                                    }
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
            Some(Err(message)) => rsx! { Notice { message: message.clone() } },
            None => rsx! { p { "Loading kernels…" } },
        }
        if adding() {
            editor::AddKernel { onclose: move |_| adding.set(false), onsaved: move |_| { adding.set(false); rows.restart(); } }
        }
        if let Some(row) = alias() {
            editor::AliasEditor { kernel: row, onclose: move |_| alias.set(None), onsaved: move |_| { alias.set(None); rows.restart(); } }
        }
        if !deleting().is_empty() {
            Confirm { title: "Delete kernel?", description: format!("Permanently remove {} from the server's kernel directory?", deleting()), label: "Delete kernel", onclose: move |_| deleting.set(String::new()),
                onconfirm: move |_| {
                    let name = deleting(); deleting.set(String::new()); busy.set(true); error.set(String::new());
                    spawn(async move {
                        match request("DELETE", &format!("/v1/kernels/{}", encode(&name)), None, &auth.csrf()).await {
                            Ok(_) => rows.restart(), Err(message) => error.set(message),
                        }
                        busy.set(false);
                    });
                }
            }
        }
    }
}
