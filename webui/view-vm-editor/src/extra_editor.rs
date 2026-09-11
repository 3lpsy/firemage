use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use serde_json::{Value, json};

#[component]
pub fn StorageEditor(
    spec: Value,
    onclose: EventHandler<()>,
    onapply: EventHandler<Value>,
) -> Element {
    let text = use_signal(|| {
        crate::spec::to_toml(&json!({"initrd":spec["initrd"], "drives":spec["drives"].as_array().cloned().unwrap_or_default()})).unwrap_or_default()
    });
    let mut error = use_signal(String::new);
    rsx! {
        Modal { title: "Configure initrd and additional drives", onclose,
            form { class: "modal-form", onsubmit: move |event| {
                event.prevent_default(); event.stop_propagation();
                let value = match parse(&text()) { Ok(value) => value, Err(message) => { error.set(message); return; } };
                let mut updated = spec.clone();
                updated["initrd"] = value["initrd"].clone();
                updated["drives"] = value["drives"].clone();
                onapply.call(updated);
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    Editor { label: "Storage TOML", id: "vm-storage-toml", value: text, rows: 16 }
                    p { class: "small muted", "Use an optional [initrd] table and [[drives]] entries with id, asset and read_only. Local paths must be inside allowed host asset roots." }
                    pre { class: "vm-storage-example", "[[drives]]\nid = \"data\"\nread_only = false\n[drives.asset]\nkind = \"local\"\npath = \"/var/lib/firemage/assets/data.ext4\"" }
                }
                div { class: "actions end",
                    button { r#type: "button", onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", "Apply to draft" }
                }
            }
        }
    }
}

fn parse(text: &str) -> Result<Value, String> {
    let mut value: Value = toml::from_str(text).map_err(|error| error.to_string())?;
    if value.as_object().is_none_or(|map| {
        map.keys()
            .any(|key| !matches!(key.as_str(), "initrd" | "drives"))
    }) {
        return Err("Storage supports only initrd and drives.".into());
    }
    if !value["initrd"].is_null() {
        let asset: firemage_wire::Asset =
            serde_json::from_value(value["initrd"].clone()).map_err(|error| error.to_string())?;
        asset.validate().map_err(|error| error.to_string())?;
    }
    if value["drives"].is_null() {
        value["drives"] = json!([]);
    }
    let _: Vec<firemage_wire::Drive> =
        serde_json::from_value(value["drives"].clone()).map_err(|error| error.to_string())?;
    Ok(value)
}
