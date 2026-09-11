//! Owner-scoped write-only credentials for proxy injection and guest environments.
mod editor;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, get, request, text, timestamp};
use firemage_webui_provider_auth::use_auth;

#[component]
pub fn Secrets() -> Element {
    let auth = use_auth();
    let mut refresh = use_signal(|| 0u32);
    let mut editing = use_signal(|| None::<String>);
    let mut deleting = use_signal(String::new);
    let mut error = use_signal(String::new);
    let rows = use_resource(move || {
        let _ = refresh();
        async { get("/v1/secrets").await }
    });
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "ACCESS" }
                h1 { "Secrets" }
                p { class: "muted", "Named credentials for your VMs and their upstream requests." }
            }
            button { class: "primary", onclick: move |_| editing.set(Some(String::new())), "+ Create secret" }
        }
        div { class: "policy-guide",
            p { "Values are write-only. VM definitions store secret names, never their values." }
            Info { title: "Secrets and guest access",
                p { "Each account owns its secrets. Create a secret once and refer to its name in a VM environment variable, HTTP header, request signature, or upstream proxy credential." }
                p { "Proxy credentials stay on the host and are added after a request passes the VM’s policy. Environment secrets are delivered to the guest, so guest processes can read them. Replacing a secret never reveals its previous value." }
            }
        }
        Notice { message: error() }
        match rows.read().as_ref() {
            Some(Ok(value)) => rsx! {
                if value.as_array().is_none_or(Vec::is_empty) {
                    Empty { title: "No secrets", description: "Add a named credential to use it in a VM’s environment or egress policy." }
                } else {
                    table {
                        thead { tr { th { "NAME" } th { "UPDATED" } th { "" } } }
                        tbody {
                            for row in value.as_array().into_iter().flatten() {
                                tr {
                                    td { strong { r#"{text(row, "name")}"# } }
                                    td { r#"{timestamp(&row["updated_at"])}"# }
                                    td { div { class: "actions end",
                                        button { onclick: { let name = text(row, "name"); move |_| editing.set(Some(name.clone())) }, "Replace value" }
                                        button { class: "danger subtle", onclick: { let name = text(row, "name"); move |_| deleting.set(name.clone()) }, "Delete" }
                                    } }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(error)) => rsx! { Notice { message: error.clone() } },
            None => rsx! { p { class: "muted", "Loading secrets…" } },
        }
        if let Some(name) = editing() {
            editor::SecretEditor { name, onclose: move |_| editing.set(None), onsaved: move |_| { editing.set(None); refresh += 1; } }
        }
        if !deleting().is_empty() {
            Confirm { title: "Delete secret?", description: format!("Remove {}? VM operations that require this credential will fail until it is replaced or its references are removed.", deleting()), label: "Delete secret",
                onclose: move |_| deleting.set(String::new()),
                onconfirm: move |_| {
                    let name = deleting(); deleting.set(String::new());
                    spawn(async move {
                        match request("DELETE", &format!("/v1/secrets/{}", encode(&name)), None, &auth.csrf()).await {
                            Ok(_) => refresh += 1, Err(message) => error.set(message),
                        }
                    });
                }
            }
        }
    }
}
