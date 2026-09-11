//! Shared full-page VM definition form and draft-only section editors.
mod draft;
mod egress;
mod extra_editor;
mod fields;
mod guided;
mod kernel;
mod machine;
mod network;
mod registry;
mod registry_fields;
mod sections;
mod spec;
mod state;
mod storage;
mod subeditors;
mod workload;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn VmEditor(
    #[props(default)] vm: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<String>,
) -> Element {
    let auth = use_auth();
    let existing = vm["id"].as_str().map(str::to_owned);
    let initial = vm["spec"].clone();
    let fields = state::use_fields(&initial);
    let mut base = use_signal(|| initial.clone());
    let mut advanced = use_signal(|| false);
    let mut toml_text = use_signal(|| spec::to_toml(&initial).unwrap_or_default());
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut policy_creating = use_signal(|| false);
    let mut subeditor = use_signal(|| None::<(String, Value)>);
    use_effect(move || {
        if !error().is_empty()
            && let Some(alert) = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| {
                    document
                        .query_selector(".vm-editor-page [role='alert']")
                        .ok()
                        .flatten()
                })
        {
            alert.scroll_into_view();
        }
    });
    let is_existing = existing.is_some();
    let vm_id = existing.clone().unwrap_or_default();
    let onconfigure = move |kind: String| {
        if kind == "egress" {
            policy_creating.set(true);
            return;
        }
        match fields.value(&base(), true) {
            Ok(value) => {
                subeditor.set(Some((kind, value)));
                error.set(String::new());
            }
            Err(message) => error.set(message),
        }
    };
    if policy_creating() {
        return rsx! { firemage_webui_view_egress::PolicyEditor { selecting: true,
            onclose: move |_| policy_creating.set(false),
            onsaved: move |policy: Value| {
                let mut value = base();
                if !value.is_object() { value = serde_json::json!({}); }
                value["egress_policy"] = policy["id"].clone();
                value.as_object_mut().unwrap().remove("egress");
                base.set(value); policy_creating.set(false);
            },
        } };
    }
    rsx! {
        section { class: "vm-editor-page",
            div { class: "vm-page-breadcrumb",
                a { href: "#vms", "Virtual machines" } span { "/" }
                if is_existing { a { href: "#vms/{vm_id}", "{text(&vm[\"spec\"], \"name\")}" } span { "/" } }
                span { if is_existing { "Edit" } else { "Create" } }
            }
            div { class: "heading", h1 { if is_existing { "Edit virtual machine" } else { "Create virtual machine" } } }
            form { class: "vm-editor-form", novalidate: true,
                onsubmit: move |event| {
                    event.prevent_default();
                    if busy() { return; }
                    let value = if advanced() { toml::from_str::<Value>(&toml_text()).map_err(|e| e.to_string()) } else { fields.value(&base(), false) };
                    let value = match value.and_then(|value| draft::validate(value, &initial)) {
                        Ok(value) => value, Err(message) => { error.set(message); return; }
                    };
                    let path = existing.as_ref().map(|id| format!("/v1/vms/{id}")).unwrap_or("/v1/vms".into());
                    busy.set(true); error.set(String::new());
                    spawn(async move {
                        match request(if is_existing { "PUT" } else { "POST" }, &path, Some(value), &auth.csrf()).await {
                            Ok(saved) => onsaved.call(text(&saved, "id")),
                            Err(message) => error.set(message),
                        }
                        busy.set(false);
                    });
                },
                div { class: "tabs vm-editor-tabs",
                    button { r#type: "button", class: if !advanced() { "active" } else { "" }, disabled: busy(),
                        onclick: move |_| {
                            if advanced() {
                                match toml::from_str::<firemage_wire::VmSpec>(&toml_text()).map_err(|e| e.to_string()).and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())) {
                                    Ok(value) => { fields.load(&value); base.set(value); advanced.set(false); error.set(String::new()); }
                                    Err(message) => error.set(message),
                                }
                            }
                        }, "Guided setup"
                    }
                    button { r#type: "button", class: if advanced() { "active" } else { "" }, disabled: busy(),
                        onclick: move |_| {
                            if !advanced() {
                                match fields.value(&base(), true).and_then(|value| spec::to_toml(&value)) {
                                    Ok(text) => { toml_text.set(text); advanced.set(true); error.set(String::new()); }
                                    Err(message) => error.set(message),
                                }
                            }
                        }, "Full TOML"
                    }
                }
                Notice { message: error() }
                fieldset { class: "vm-editor-body", disabled: busy(),
                    if advanced() {
                        Editor { label: "VM configuration", id: "vm-toml", value: toml_text, rows: 30 }
                    } else {
                        guided::Guided { fields, base, existing: is_existing, vm_id, owner: vm["owner_id"].as_str().map(str::to_owned).unwrap_or_else(|| text(&auth.session.read()["user"],"id")), onconfigure }
                    }
                }
                div { class: "vm-editor-footer",
                    p { class: "small muted", if is_existing { "Changes apply on the next boot." } else { "Creates a definition. Start the VM separately." } }
                    div { class: "actions end",
                        button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                        button { class: "primary", r#type: "submit", disabled: busy(),
                            if busy() { "Saving…" } else if is_existing { "Save configuration" } else { "Create VM" }
                        }
                    }
                }
            }
        }
        if let Some((kind, value)) = subeditor() {
            subeditors::Subeditors { kind: kind.clone(), spec: value,
                onclose: move |_| subeditor.set(None),
                onapply: move |value: Value| { fields.apply_section(&kind, &value); base.set(value); subeditor.set(None); },
            }
        }
    }
}
#[cfg(test)]
mod tests;
