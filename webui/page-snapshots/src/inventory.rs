use dioxus::prelude::*;
use firemage_webui_component_controls::Icon;
use firemage_webui_provider_api::{text, timestamp};
use serde_json::Value;

pub fn navigate(id: &str) {
    if let Some(window) = web_sys::window() {
        let path = if id.is_empty() {
            "snapshots".into()
        } else {
            format!("snapshots/{id}")
        };
        let _ = window.location().set_hash(&path);
    }
}

#[component]
pub fn Inventory(
    snapshots: Vec<Value>,
    #[props(default)] selected: String,
    #[props(default)] vm_scope: bool,
) -> Element {
    rsx! {
        table { class: "snapshot-table",
            thead { tr { th { "Alias" } if !vm_scope { th { "Source VM" } } th { "Saved" } th { "Size" } th { "aria-label": "Details" } } }
            tbody {
                for snapshot in snapshots {
                    tr { key: "{text(&snapshot, \"id\")}", class: if text(&snapshot, "id") == selected { "vm-row selected" } else { "vm-row" },
                        onclick: { let target = if text(&snapshot, "id") == selected { String::new() } else { text(&snapshot, "id") }; move |_| navigate(&target) },
                        td { a { class: "table-link", href: format!("#snapshots/{}", text(&snapshot, "id")),
                            "aria-expanded": (text(&snapshot, "id") == selected).to_string(),
                            onclick: move |event| event.stop_propagation(), "{text(&snapshot, \"alias\")}" } }
                        if !vm_scope { td { "{text(&snapshot, \"source_vm_name\")}" } }
                        td { "{timestamp(&snapshot[\"created_at\"])}" }
                        td { "{crate::model::size(snapshot[\"size_bytes\"].as_u64().unwrap_or(0))}" }
                        td { class: "vm-row-chevron",
                            button { class: "icon-button", title: "Toggle snapshot details", "aria-label": format!("Toggle {} details", text(&snapshot, "alias")),
                                "aria-expanded": (text(&snapshot, "id") == selected).to_string(),
                                onclick: { let target = if text(&snapshot, "id") == selected { String::new() } else { text(&snapshot, "id") }; move |event| { event.stop_propagation(); navigate(&target); } },
                                Icon { name: "chevron-right", size: 16 }
                            }
                        }
                    }
                }
            }
        }
    }
}
