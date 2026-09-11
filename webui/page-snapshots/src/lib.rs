mod controls;
mod drawer;
mod model;
mod requirements;
mod restore;
mod save;
mod upload;
pub use controls::VmSnapshots;
use dioxus::prelude::*;
use firemage_webui_component_controls::{Empty, Notice};
use firemage_webui_provider_api::{text, timestamp};
use firemage_webui_provider_auth::use_auth;

#[component]
pub fn Snapshots() -> Element {
    let auth = use_auth();
    let mut rows = use_resource(model::snapshots);
    let mut selected = use_signal(String::new);
    let mut search = use_signal(String::new);
    let mut uploading = use_signal(|| false);
    let mut saving = use_signal(|| false);
    rsx! {
        style { {include_str!("style.css")} }
        div { class: "snapshot-page",
            div { class: "heading",
                h1 { "Snapshots" }
                div { class: "actions",
                    button { onclick: move |_| rows.restart(), "Refresh snapshots" }
                    if auth.is_admin() {
                        button { onclick: move |_| uploading.set(true), "Upload snapshot" }
                        button { class: "primary", onclick: move |_| saving.set(true), "Save VM snapshot" }
                    }
                }
            }
            div { class: "toolbar",
                input { id: "snapshot-search", r#type: "search", placeholder: "Search snapshots or source VM", "aria-label": "Search snapshots", value: r#"{search}"#, oninput: move |event| search.set(event.value()) }
            }
            match rows.read().as_ref() {
                Some(Ok(snapshots)) => {
                    let query = search().to_lowercase();
                    let filtered = snapshots.iter().filter(|snapshot| text(snapshot, "alias").to_lowercase().contains(&query) || text(snapshot, "source_vm_name").to_lowercase().contains(&query));
                    let detail = snapshots.iter().find(|snapshot| text(snapshot, "id") == selected()).cloned();
                    rsx! {
                        div { class: if detail.is_some() { "snapshot-catalog with-drawer" } else { "snapshot-catalog" },
                            div {
                                if snapshots.is_empty() { Empty { title: "No snapshots yet", description: "Save a paused VM or upload a snapshot bundle." } }
                                else {
                                    table { class: "snapshot-table",
                                        thead { tr { th { "Alias" } th { "Source VM" } th { "Saved" } th { "Size" } } }
                                        tbody {
                                            for snapshot in filtered {
                                                tr { key: r#"{text(snapshot, "id")}"#, class: if text(snapshot, "id") == selected() { "selected" },
                                                    onclick: { let id = text(snapshot, "id"); move |_| selected.set(id.clone()) },
                                                    td { button { class: "quiet", r#"{text(snapshot, "alias")}"# } }
                                                    td { r#"{text(snapshot, "source_vm_name")}"# }
                                                    td { r#"{timestamp(&snapshot["created_at"])}"# }
                                                    td { r#"{model::size(snapshot["size_bytes"].as_u64().unwrap_or(0))}"# }
                                                }
                                            }
                                        }
                                    }
                                    p { class: "muted small", "Snapshots remain available after their source VM is deleted." }
                                }
                            }
                            if let Some(snapshot) = detail.clone() {
                                drawer::Drawer { key: r#"{text(&snapshot, "id")}"#, snapshot, onclose: move |_| selected.set(String::new()), onchanged: move |_| rows.restart() }
                            }
                        }
                    }
                },
                Some(Err(error)) => rsx! { Notice { message: error.clone() } },
                None => rsx! { p { role: "status", "Loading snapshots…" } },
            }
            if uploading() { upload::Upload { onclose: move |_| uploading.set(false), onsaved: move |id: String| { uploading.set(false); selected.set(id); rows.restart(); } } }
            if saving() { save::Save { onclose: move |_| saving.set(false), onsaved: move |id: String| { saving.set(false); selected.set(id); rows.restart(); } } }
        }
    }
}

#[cfg(test)]
mod tests;
