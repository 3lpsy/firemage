use dioxus::prelude::*;
use firemage_webui_component_controls::{Icon, Notice};
use firemage_webui_provider_api::{get, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn Egress(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let id = text(&vm, "id");
    let mut selected = use_signal(|| text(&vm["spec"], "egress_policy"));
    let mut refresh = use_signal(|| 0u32);
    let mut error = use_signal(String::new);
    let mut success = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let status_id = id.clone();
    let status = use_resource(move || {
        let id = status_id.clone();
        let _ = refresh();
        async move { get(&format!("/v1/vms/{id}/egress")).await }
    });
    let network = text(&vm["spec"]["network"], "network");
    let owner = text(&vm, "owner_id");
    let networks = use_resource(|| async { get("/v1/networks").await });
    let inferred = networks
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|v| v.as_array())
        .and_then(|rows| {
            rows.iter()
                .find(|r| text(r, "id") == network)
                .or_else(|| rows.iter().find(|r| text(r, "name") == network))
        })
        .is_some_and(|r| r["policy"]["mode"] == "firemage-only");
    let restricted = status
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|v| v["restricted_network"].as_bool())
        .unwrap_or(inferred);
    let live = matches!(text(&vm, "state").as_str(), "running" | "paused");
    rsx! {
        div { class: "heading compact vm-tab-heading", h3 { "Egress" }
            button { onclick: move |_| refresh += 1, "Refresh egress" }
        }
        Notice { message: error() } Notice { message: success(), success: true }
        if auth.is_admin() && restricted {
            form { onsubmit: move |event| {
                event.prevent_default(); if busy() { return; }
                busy.set(true); error.set(String::new()); success.set(String::new());
                let path = format!("/v1/vms/{id}/egress-policy");
                let body = json!({"policy_id":if selected().is_empty() { Value::Null } else { json!(selected()) }});
                spawn(async move {
                    match request("PUT",&path,Some(body),&auth.csrf()).await {
                        Ok(_) => { success.set("Egress policy applied.".into()); refresh += 1; onchanged.call(()); },
                        Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                fieldset { class: "vm-editor-body", disabled: busy(),
                    crate::selector::CatalogSelect { kind: "policies", label: "Egress policy", id: "vm-live-egress-policy", selected: selected(), owner, onchange: move |id| selected.set(id) }
                    div { class: "actions wrap",
                        a { class: "button subtle", href: "#egress/policies/new", Icon { name: "plus", size: 15 } "Create policy" }
                        if !selected().is_empty() { a { class: "button subtle", href: "#egress/policies/{selected}/edit", "Edit shared policy" } }
                        button { class: "primary", r#type: "submit", if busy() { "Applying…" } else { "Apply policy" } }
                    }
                    p { class: "small muted", if live { "Applies now. Existing egress connections close so the selected policy takes effect." } else { "The selected policy will be used when this VM starts." } }
                }
            }
        } else if !restricted { p { class: "small muted", "Controlled egress requires a Firemage-only network." } }
        match status.read().as_ref() {
            Some(Ok(value)) => rsx! { crate::summary::Summary { value: value.clone() } },
            Some(Err(message)) => rsx! { Notice { message: message.clone() } },
            None => rsx! { p { "Loading egress…" } },
        }
    }
}
