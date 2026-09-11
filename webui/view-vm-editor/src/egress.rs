use crate::fields::Fields;
use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use firemage_webui_provider_api::{get, text};
use serde_json::{Value, json};

#[component]
pub fn EgressFields(
    fields: Fields,
    mut base: Signal<Value>,
    oncreate: EventHandler<()>,
    owner: String,
) -> Element {
    let auth = firemage_webui_provider_auth::use_auth();
    let can_create = owner == text(&auth.session.read()["user"], "id");
    let networks = use_resource(|| async { get("/v1/networks").await });
    let name = (fields.network)();
    let selected = text(&base(), "egress_policy");
    let restricted = networks
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|rows| rows.as_array())
        .and_then(|rows| {
            rows.iter()
                .find(|row| text(row, "id") == name)
                .or_else(|| rows.iter().find(|row| text(row, "name") == name))
        })
        .is_some_and(|row| row["policy"]["mode"] == "firemage-only");
    rsx! {
        if restricted && (fields.mode)() != "socket" {
            firemage_webui_view_egress::CatalogSelect { key: "policy-{selected}", kind: "policies", label: "Egress policy", id: "vm-egress-policy", selected: selected.clone(), owner,
                onchange: move |id: String| {
                    let mut value = base(); if !value.is_object() { value = json!({}); }
                    value["egress_policy"] = if id.is_empty() { Value::Null } else { json!(id) };
                    value.as_object_mut().unwrap().remove("egress"); base.set(value);
                }, oncreate: if can_create { Some(oncreate) } else { None },
            }
            if !selected.is_empty() { a { class: "button subtle", href: "#egress/policies/{selected}", target: "_blank", rel: "noopener", "View shared policy" } }
            p { class: "small muted", "Uses the latest shared policy. Policy and upstream proxy edits apply while the VM runs." }
        } else {
            p { class: "small muted", "Choose a Firemage-only network to select a controlled egress policy." }
            if !selected.is_empty() {
                p { class: "small", "This draft still references an egress policy. Choose a Firemage-only network or clear the policy before saving." }
                button { r#type: "button", onclick: move |_| { let mut value = base(); value.as_object_mut().unwrap().remove("egress_policy"); base.set(value); }, "Clear egress policy" }
            }
            if let Some(Err(message)) = networks.read().as_ref() { Notice { message: message.clone() } }
        }
    }
}
