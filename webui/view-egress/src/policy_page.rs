use crate::{
    catalog_model, model,
    policy_sections::{self, Section},
    selector::CatalogSelect,
};
use dioxus::prelude::*;
use firemage_webui_component_controls::{Field, Notice};
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn PolicyEditor(
    #[props(default)] value: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<Value>,
    #[props(default)] selecting: bool,
) -> Element {
    let auth = use_auth();
    let alias = use_signal(|| text(&value, "alias"));
    let mut draft = use_signal(|| {
        let mut draft = model::Draft::new(&value["policy"]);
        draft.upstream = Value::Null;
        draft.inherit_upstream = value["policy"]["inherit_upstream"]
            .as_bool()
            .unwrap_or(false);
        draft
    });
    let mut upstream = use_signal(|| {
        value["upstream_proxy_id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if draft.read().inherit_upstream {
                    "default".into()
                } else {
                    String::new()
                }
            })
    });
    let ca = use_signal(|| text(&value["policy"]["http"], "upstream_ca_pem"));
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut proxy_editor = use_signal(|| false);
    let mut refresh = use_signal(|| 0u32);
    let existing = value["id"].is_string();
    let count = catalog_model::usage(&value, "vm");
    let live = catalog_model::running(&value);
    let owner = value["owner_id"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| text(&auth.session.read()["user"], "id"));
    let can_create_proxy = owner == text(&auth.session.read()["user"], "id");
    if !auth.is_admin() {
        return rsx! { Notice { message: "Administrator access is required to edit egress policies." } };
    }
    rsx! {
        section { class: "vm-editor-page egress-editor-page",
            style { {include_str!("catalog.css")} }
            div { class: "vm-page-breadcrumb",
                button { class: "quiet", disabled: busy(), onclick: move |_| onclose.call(()), if selecting { "Back to VM draft" } else { "Egress policies" } }
                span { "/" } span { if existing { "Edit" } else { "Create" } }
            }
            div { class: "heading", h1 { if existing { "Edit egress policy" } else { "Create egress policy" } } }
            form { class: "vm-editor-form", onsubmit: move |event| {
                event.prevent_default();
                if busy() { return; }
                let body = draft.read().policy().and_then(|policy| catalog_model::policy_input(&alias(), policy, &upstream(), &ca()));
                let mut body = match body { Ok(v) => v, Err(message) => { error.set(message); return; } };
                let path = if existing { body["revision"] = value["revision"].clone(); format!("/v1/egress/policies/{}",text(&value,"id")) } else { "/v1/egress/policies".into() };
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request(if existing { "PUT" } else { "POST" }, &path, Some(body), &auth.csrf()).await {
                        Ok(saved) => onsaved.call(saved), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                Notice { message: error() }
                fieldset { class: "vm-editor-body", disabled: busy(),
                    div { class: "vm-form-layout", policy_sections::Index {}
                        div { class: "vm-form-sections",
                            Section { id: "identity", title: "Identity", summary: alias(), open: true,
                                Field { label: "Alias", id: "policy-alias", value: alias, required: true, placeholder: "model-review" }
                                p { class: "small muted", "Unique within the owner account. VMs keep their reference when the alias changes." }
                            }
                            policy_sections::HttpSection { draft }
                            Section { id: "tunnels", title: "TCP tunnels", summary: format!("{} tunnels",draft.read().tunnels.len()), policy_sections::Tunnels { draft } }
                            Section { id: "upstream", title: "Upstream proxy", summary: if upstream().is_empty() { "Direct" } else if upstream() == "default" { "Server default" } else { "Shared proxy" }, open: true,
                                for version in [refresh()] {
                                    CatalogSelect { key: "proxy-{version}", kind: "proxies", label: "Upstream proxy", id: "policy-upstream", selected: upstream(), upstream: true, owner: owner.clone(),
                                        onchange: move |id| { upstream.set(id); draft.write().inherit_upstream = upstream() == "default"; }, oncreate: if can_create_proxy { Some(EventHandler::new(move |_| proxy_editor.set(true))) } else { None },
                                    }
                                }
                                p { class: "small muted", "VM → Firemage → upstream proxy → allowed destination. Proxy failures never fall back to direct access." }
                            }
                            Section { id: "tls", title: "Destination TLS", summary: if ca().is_empty() { "System trust" } else { "Custom certificates" }, policy_sections::Tls { ca } }
                        }
                    }
                }
                if existing && count > 0 {
                    p { class: "notice", "Updates {count} associated VMs, including {live} running or paused. Existing egress connections close so new rules take effect. Guest programs reconnect." }
                } else if selecting {
                    p { class: "small muted", "Creates a reusable policy and selects it in your VM draft. It remains available if you cancel VM creation." }
                }
                div { class: "vm-editor-footer",
                    p { class: "small muted", "All unmatched destinations are denied." }
                    div { class: "actions end",
                        button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                        button { r#type: "submit", class: "primary", disabled: busy(),
                            if busy() { "Applying…" } else if existing && live > 0 { "Save and apply to {live} live VMs" } else if existing { "Save policy" } else if selecting { "Create and select" } else { "Create policy" }
                        }
                    }
                }
            }
            if proxy_editor() {
                crate::proxy_editor::ProxyEditor { onclose: move |_| proxy_editor.set(false), onsaved: move |proxy: Value| { upstream.set(text(&proxy,"id")); refresh += 1; proxy_editor.set(false); } }
            }
        }
    }
}
