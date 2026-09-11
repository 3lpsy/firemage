mod controls;
mod drawer;
mod inventory;
mod model;
mod requirements;
mod restore;
mod save;
mod upload;
pub use controls::VmSnapshots;
use dioxus::prelude::*;
use firemage_webui_component_controls::{Empty, Notice};
use firemage_webui_provider_api::text;
use firemage_webui_provider_auth::use_auth;

#[component]
pub fn Snapshots() -> Element {
    let auth = use_auth();
    let mut rows = use_resource(model::snapshots);
    let selected = firemage_webui_routes::use_snapshot_id();
    let mut search = use_signal(String::new);
    let mut uploading = use_signal(|| false);
    let mut saving = use_signal(|| false);
    rsx! {
        style { {include_str!("style.css")} }
        div { class: "snapshot-page",
            div { class: "heading",
                div {
                    div { class: "eyebrow", "COMPUTE" }
                    h1 { "Snapshots" }
                }
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
                    let filtered = snapshots.iter().filter(|snapshot| text(snapshot, "alias").to_lowercase().contains(&query) || text(snapshot, "source_vm_name").to_lowercase().contains(&query)).cloned().collect();
                    let detail = snapshots.iter().find(|snapshot| text(snapshot, "id") == selected()).cloned();
                    rsx! {
                        div { class: if detail.is_some() { "snapshot-catalog with-drawer" } else { "snapshot-catalog" },
                            div {
                                if snapshots.is_empty() { Empty { title: "No snapshots yet", description: "Save a paused VM or upload a snapshot bundle." } }
                                else {
                                    inventory::Inventory { snapshots: filtered, selected: selected() }
                                    p { class: "muted small", "Snapshots remain available after their source VM is deleted." }
                                }
                            }
                            for snapshot in detail.clone() {
                                drawer::Drawer { key: r#"{text(&snapshot, "id")}"#, snapshot, onclose: move |_| inventory::navigate(""), onchanged: move |_| rows.restart() }
                            }
                        }
                    }
                },
                Some(Err(error)) => rsx! { Notice { message: error.clone() } },
                None => rsx! { p { role: "status", "Loading snapshots…" } },
            }
            if uploading() { upload::Upload { onclose: move |_| uploading.set(false), onsaved: move |id: String| { uploading.set(false); inventory::navigate(&id); rows.restart(); } } }
            if saving() { save::Save { onclose: move |_| saving.set(false), onsaved: move |id: String| { saving.set(false); inventory::navigate(&id); rows.restart(); } } }
        }
    }
}

#[cfg(test)]
mod tests;
