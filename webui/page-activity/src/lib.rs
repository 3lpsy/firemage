//! Recorded control-plane changes.
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{get, timestamp};
#[component]
pub fn Activity() -> Element {
    let mut refresh = use_signal(|| 0);
    let rows = use_resource(move || {
        let _ = refresh();
        async { get("/v1/activity").await }
    });
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "OBSERVABILITY" }
                h1 { "Activity" }
                p { class: "muted", "Recent operations recorded by this server." }
            }
            button { onclick: move |_| refresh += 1, "Refresh" }
        }
        match rows.read().as_ref() {
            Some(Ok(value)) => rsx! {
                if value.as_array().is_none_or(Vec::is_empty) {
                    Empty {
                        title: "No activity yet",
                        description: "VM, network, account, and configuration changes appear here.",
                    }
                } else {
                    table {
                        thead {
                            tr {
                                th { "TIME" }
                                th { "ACTOR" }
                                th { "ACTION" }
                                th { "RESOURCE" }
                            }
                        }
                        tbody {
                            for row in value.as_array().into_iter().flatten() {
                                tr {
                                    td { class: "mono small", {timestamp(&row["at"])} }
                                    td { r#"{row["actor"].as_str().unwrap_or_default()}"# }
                                    td {
                                        strong { r#"{row["action"].as_str().unwrap_or_default()}"# }
                                    }
                                    td { class: "mono small", r#"{row["resource"].as_str().unwrap_or_default()}"# }
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
                p { class: "muted", "Loading activity…" }
            },
        }
    }
}
