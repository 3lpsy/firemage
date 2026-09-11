use crate::mutation::{self, Change, EditorMode};
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::text;
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Environment(vm: Value, onchanged: EventHandler<()>) -> Element {
    rsx! {
        style { {include_str!("style.css")} }
        for id in [text(&vm, "id")] {
            Panel { key: "{id}", vm: vm.clone(), onchanged }
        }
    }
}

#[component]
fn Panel(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut editing = use_signal(|| None::<EditorMode>);
    let mut deleting = use_signal(|| None::<(String, Value)>);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let can_edit = matches!(vm["state"].as_str(), Some("defined" | "stopped" | "failed"));
    rsx! {
        div { class: "heading compact",
            h3 { "Guest environment"
                Info { title: "Environment and sensitive values",
                    p { "Plain values are visible in the VM configuration. Sensitive entries refer to a named secret, which Firemage resolves when preparing the guest. The guest receives the actual value and can read it." }
                    p { "Use HTTP header injection or request signing to keep an API credential outside the guest. Environment variables are appropriate when the guest application must have the credential itself." }
                }
            }
            if auth.is_admin() {
                button { disabled: !can_edit || busy(), onclick: move |_| editing.set(Some(EditorMode::Add)), "Add Variable" }
            }
        }
        Notice { message: error() }
        if vm["spec"]["environment"].as_object().is_none_or(serde_json::Map::is_empty) {
            p { class: "small muted", "No environment variables configured." }
        } else {
            div { class: "table-scroll",
                table { class: "environment-table",
                    thead { tr { th { "Name" } th { "Value" } if auth.is_admin() { th { "Actions" } } } }
                    tbody {
                        for (name, value) in vm["spec"]["environment"].as_object().into_iter().flatten() {
                            tr { key: "{name}", "data-environment-name": name.clone(),
                                td { class: "mono small", "{name}" }
                                td {
                                    if let Some(secret) = value["secret"].as_str() {
                                        span { class: "small purple", "Secret: {secret}" }
                                    } else { code { "{value.as_str().unwrap_or_default()}" } }
                                }
                                if auth.is_admin() {
                                    td { div { class: "actions end",
                                        button { disabled: !can_edit || busy(), "aria-label": "Edit environment variable {name}",
                                            onclick: { let name = name.clone(); let value = value.clone(); move |_| editing.set(Some(EditorMode::Edit { name: name.clone(), value: value.clone() })) }, "Edit" }
                                        button { class: "danger subtle", disabled: !can_edit || busy(), "aria-label": "Delete environment variable {name}",
                                            onclick: { let name = name.clone(); let value = value.clone(); move |_| deleting.set(Some((name.clone(), value.clone()))) }, "Delete" }
                                    } }
                                }
                            }
                        }
                    }
                }
            }
        }
        if auth.is_admin() && !can_edit { p { class: "small muted", "Stop the VM before changing its environment." } }
        if auth.is_admin() && let Some(mode) = editing() {
            crate::variable_editor::VariableEditor { id: text(&vm, "id"), mode,
                onclose: move |_| editing.set(None), onsaved: move |_| { editing.set(None); onchanged.call(()); } }
        }
        if auth.is_admin() && let Some((name, previous)) = deleting() {
            Confirm { title: "Delete environment variable?", description: format!("Remove {name} from this VM?"), label: "Delete variable",
                onclose: move |_| deleting.set(None), onconfirm: move |_| {
                    if busy() { return; }
                    let id = text(&vm, "id");
                    let change = Change::Delete { name: name.clone(), previous: previous.clone() };
                    deleting.set(None); busy.set(true); error.set(String::new());
                    spawn(async move {
                        match mutation::save(&id, change, &auth.csrf()).await {
                            Ok(()) => onchanged.call(()), Err(message) => error.set(message),
                        }
                        busy.set(false);
                    });
                }
            }
        }
    }
}
