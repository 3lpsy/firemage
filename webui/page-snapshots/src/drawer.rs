use dioxus::prelude::*;
use firemage_webui_component_controls::{Confirm, Notice};
use firemage_webui_provider_api::{encode, request, text, timestamp};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Drawer(snapshot: Value, onclose: EventHandler<()>, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut deleting = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let id = text(&snapshot, "id");
    let alias = text(&snapshot, "alias");
    let delete_id = id.clone();
    rsx! {
        aside { class: "snapshot-drawer", "aria-label": "Snapshot details",
            div { class: "heading compact", h2 { r#"{alias}"# } button { class: "quiet", onclick: move |_| onclose.call(()), "Close" } }
            dl { class: "snapshot-facts",
                dt { "Source VM" } dd { r#"{text(&snapshot, "source_vm_name")}"# }
                dt { "Saved" } dd { r#"{timestamp(&snapshot["created_at"])}"# }
                dt { "Architecture" } dd { r#"{text(&snapshot, "architecture")}"# }
                dt { "Firecracker" } dd { r#"{text(&snapshot, "firecracker_version")}"# }
                dt { "Bundle size" } dd { r#"{crate::model::size(snapshot["size_bytes"].as_u64().unwrap_or(0))}"# }
                dt { "Trust" } dd { if snapshot["trusted"] == true { "Trusted" } else { "Required before restore" } }
            }
            crate::requirements::Requirements { snapshot: snapshot.clone() }
            if auth.is_admin() { crate::restore::Restore { snapshot: snapshot.clone(), onchanged } }
            div { class: "snapshot-section",
                div { class: "actions",
                    a { class: "button", href: format!("/v1/snapshots/{}/download", encode(&id)), download: format!(r#"{alias}.fmsnap"#), "Download bundle" }
                    if auth.is_admin() { button { class: "danger", disabled: busy(), onclick: move |_| deleting.set(true), "Delete" } }
                }
                p { class: "muted small", "Contains guest memory and disks. Treat downloads as sensitive." }
                Notice { message: error() }
            }
            if deleting() {
                Confirm { title: "Delete snapshot?", description: format!("Permanently delete {alias} and its saved guest data?"), label: "Delete snapshot", onclose: move |_| deleting.set(false),
                    onconfirm: move |_| {
                        deleting.set(false); busy.set(true); let id = delete_id.clone();
                        spawn(async move {
                            match request("DELETE", &format!("/v1/snapshots/{}", encode(&id)), None, &auth.csrf()).await {
                                Ok(_) => { onclose.call(()); onchanged.call(()); }, Err(message) => error.set(message),
                            }
                            busy.set(false);
                        });
                    }
                }
            }
        }
    }
}
