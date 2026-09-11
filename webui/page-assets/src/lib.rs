//! Uploaded file library and alias management.
mod editor;
mod upload;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, request};
use firemage_webui_provider_auth::use_auth;
use firemage_wire::FileAsset;

#[component]
pub fn Assets() -> Element {
    let auth = use_auth();
    let mut rows = use_resource(|| async {
        serde_json::from_value::<Vec<FileAsset>>(get("/v1/assets").await?)
            .map_err(|error| format!("Could not read assets: {error}"))
    });
    let mut adding = use_signal(|| false);
    let mut search = use_signal(String::new);
    let mut editing = use_signal(|| None::<FileAsset>);
    let mut deleting = use_signal(|| None::<FileAsset>);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "STORAGE" }
                h1 { "Assets" }
            }
            div { class: "actions",
                button { onclick: move |_| rows.restart(), "Refresh assets" }
                if auth.is_admin() {
                    button { class: "primary", onclick: move |_| adding.set(true), "+ Add asset" }
                }
            }
        }
        Notice { message: error() }
        div { class: "toolbar",
            input { id: "asset-search", r#type: "search", "aria-label": "Search assets", placeholder: "Search alias or filename", value: "{search}", oninput: move |event| search.set(event.value()) }
        }
        match rows.read().as_ref() {
            Some(Ok(assets)) => {
                let query = search().to_lowercase();
                let filtered: Vec<_> = assets.iter().filter(|asset| {
                    asset.alias.to_lowercase().contains(&query) || asset.filename.to_lowercase().contains(&query)
                }).collect();
                rsx! {
                    if filtered.is_empty() {
                        Empty { title: if query.is_empty() { "No assets yet" } else { "No matching assets" }, description: if query.is_empty() { "Add a file with an alias to attach it to your VMs." } else { "Try another alias or filename." } }
                    } else {
                        table { class: "kernel-table",
                            thead { tr { th { "ALIAS" } th { "FILE" } th { "SIZE" } th { "VMs" } th { "ACTIONS" } } }
                            tbody {
                                for asset in filtered {
                                    tr { key: "{asset.id}",
                                        td { "{asset.alias}" }
                                        td { class: "mono", "{asset.filename}" }
                                        td { class: "mono", {size_label(asset.size_bytes)} }
                                        td { "{asset.vm_count}" }
                                        td {
                                            if auth.is_admin() {
                                                div { class: "actions end",
                                                    button { disabled: busy(), onclick: { let asset = asset.clone(); move |_| editing.set(Some(asset.clone())) }, "Edit alias" }
                                                    button { class: "danger subtle", disabled: busy() || asset.vm_count > 0,
                                                        title: if asset.vm_count > 0 { format!("Detach from {} VM(s) before deleting", asset.vm_count) } else { "Remove asset from disk".into() },
                                                        onclick: { let asset = asset.clone(); move |_| deleting.set(Some(asset.clone())) }, "Delete"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        p { class: "small muted", "Assets attached to a VM cannot be deleted. Renaming an alias preserves attachments." }
                    }
                }
            },
            Some(Err(message)) => rsx! { Notice { message: message.clone() } },
            None => rsx! { p { role: "status", "Loading assets…" } },
        }
        if adding() && auth.is_admin() {
            upload::AddAsset { onclose: move |_| adding.set(false), onsaved: move |_| { adding.set(false); rows.restart(); } }
        }
        if let Some(asset) = editing() {
            editor::AliasEditor { asset, onclose: move |_| editing.set(None), onsaved: move |_| { editing.set(None); rows.restart(); } }
        }
        if let Some(asset) = deleting() {
            Confirm { title: "Delete asset?", description: format!("Permanently remove {} ({}) from the asset library?", asset.alias, asset.filename), label: "Delete asset", onclose: move |_| deleting.set(None),
                onconfirm: move |_| {
                    let path = format!("/v1/assets/{}", encode(&asset.id));
                    deleting.set(None); busy.set(true); error.set(String::new());
                    spawn(async move {
                        match request("DELETE", &path, None, &auth.csrf()).await {
                            Ok(_) => rows.restart(), Err(message) => { error.set(message); rows.restart(); },
                        }
                        busy.set(false);
                    });
                }
            }
        }
    }
}
fn size_label(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1_048_576 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else if bytes < 1_073_741_824 {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.1} GiB", bytes as f64 / 1_073_741_824.0)
    }
}
