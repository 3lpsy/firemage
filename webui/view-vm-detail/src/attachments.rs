use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use firemage_webui_provider_api::{encode, get, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Attachments(vm: Value, onedit: EventHandler<()>) -> Element {
    let auth = use_auth();
    let state = text(&vm, "state");
    let rows = use_resource(use_reactive((&vm,), |(vm,)| {
        let id = encode(&text(&vm, "id"));
        async move { get(&format!("/v1/vms/{id}/attachments")).await }
    }));
    rsx! {
        section { class: "vm-attached-files",
            div { class: "heading compact", h3 { "Attachments" }
                if auth.is_admin() { button { disabled: !matches!(state.as_str(), "defined" | "stopped" | "failed"), onclick: move |_| onedit.call(()), "Edit attachments" } }
            }
            p { class: "small muted", "Configured files are installed on boot. Secret contents stay hidden. Stop the VM to change attachments." }
            match rows.read().as_ref() {
                Some(Ok(rows)) => rsx! {
                    if rows.as_array().is_none_or(Vec::is_empty) { p { "No files or secrets attached." } }
                    else {
                        div { class: "table-scroll",
                            table { thead { tr { th { "Source" } th { "Alias / name" } th { "Guest destination" } th { "UID:GID" } th { "Mode" } } }
                                tbody { for row in rows.as_array().into_iter().flatten() {
                                    tr { td { "{text(row, \"kind\")}" } td { "{text(row, \"name\")}" }
                                        td { class: "mono small", "{text(row, \"destination\")}" }
                                        td { "{row[\"uid\"]}:{row[\"gid\"]}" }
                                        td { class: "mono", {format!("{:04o}", row["mode"].as_u64().unwrap_or_default())} }
                                    }
                                } }
                            }
                        }
                    }
                },
                Some(Err(error)) => rsx! { Notice { message: error.clone() } },
                None => rsx! { p { "Loading attachments…" } },
            }
        }
    }
}
