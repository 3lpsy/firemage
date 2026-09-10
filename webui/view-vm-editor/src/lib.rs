//! VM definition form with an equivalent full TOML editor.
mod fields;
mod kernel;
mod registry;
mod registry_fields;
mod spec;
mod state;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::request;
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;
#[component]
pub fn VmEditor(
    #[props(default)] vm: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
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
    let make_spec = move || {
        let generated = fields.form().spec()?;
        let mut value = spec::merge_guided(&base(), generated);
        value["attachments"] = serde_json::to_value(
            firemage_webui_view_asset_attachments::attachments(&fields.attachments.read())?,
        )
        .map_err(|e| e.to_string())?;
        Ok::<_, String>(value)
    };
    let is_existing = existing.is_some();
    rsx! {
        Modal {
            title: if is_existing { "Configure virtual machine" } else { "Create virtual machine" },
            onclose: move |_| { if !busy() { onclose.call(()); } },
            form {
                class: "modal-form",
                onsubmit: move |event| {
                    event.prevent_default();
                    if busy() {
                        return;
                    }
                    let spec = if advanced() {
                        toml::from_str::<Value>(&toml_text()).map_err(|e| e.to_string())
                    } else {
                        make_spec()
                    };
                    let spec = match spec {
                        Ok(s) => s,
                        Err(e) => {
                            error.set(e);
                            return;
                        }
                    };
                    let path = existing
                        .as_ref()
                        .map(|id| format!("/v1/vms/{id}"))
                        .unwrap_or("/v1/vms".into());
                    busy.set(true);
                    error.set(String::new());
                    spawn(async move {
                        match request(
                                if is_existing { "PUT" } else { "POST" },
                                &path,
                                Some(spec),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(_) => onsaved.call(()),
                            Err(e) => error.set(e),
                        }
                        busy.set(false);
                    });
                },
                div { class: "modal-form-body",
                    div { class: "tabs",
                        button {
                            r#type: "button",
                            class: if !advanced() { "active" } else { "" },
                            disabled: busy(),
                            onclick: move |_| {
                                if advanced() {
                                    match toml::from_str::<firemage_wire::VmSpec>(&toml_text()).map_err(|e| e.to_string()).and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())) {
                                        Ok(value) => { fields.load(&value); base.set(value); advanced.set(false); error.set(String::new()); },
                                        Err(e) => error.set(e.to_string()),
                                    }
                                }
                            },
                            "Guided setup"
                        }
                        button {
                            r#type: "button",
                            class: if advanced() { "active" } else { "" },
                            onclick: move |_| {
                                if !advanced() {
                                    match fields.form().draft().and_then(|generated| {
                                        let mut value = spec::merge_guided(&base(), generated);
                                        value["attachments"] = serde_json::to_value(firemage_webui_view_asset_attachments::attachments(&fields.attachments.read())?).map_err(|e| e.to_string())?;
                                        spec::to_toml(&value)
                                    }) {
                                        Ok(s) => toml_text.set(s),
                                        Err(message) => { error.set(message); return; }
                                    }
                                }
                                advanced.set(true);
                            },
                            "Full TOML"
                        }
                    }
                    Notice { message: error() }
                    if advanced() {
                        Editor {
                            label: "VM configuration",
                            id: "vm-toml",
                            value: toml_text,
                            rows: 20,
                        }
                        p { class: "muted small",
                            "All VM settings are available here: assets, registry access, drives, network, userdata, boot files, metadata, host isolation, process limits, and external sockets. Isolation mode is fixed at creation."
                        }
                    } else {
                        fields::Guided {
                            fields,
                            existing: is_existing,
                        }
                    }
                }
                p { class: "small muted", if is_existing { "Changes apply on the next boot." } else { "Creates a VM definition. Start the VM separately." } }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move | _ | onclose
                                                        .call(()), "Cancel" }
                    button {
                        class: "primary",
                        r#type: "submit",
                        disabled: busy(),
                        if busy() {
                            "Saving…"
                        } else if is_existing {
                            "Save configuration"
                        } else {
                            "Create VM"
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
