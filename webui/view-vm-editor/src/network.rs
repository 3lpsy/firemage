use crate::fields::Fields;
use dioxus::prelude::*;
use firemage_webui_component_controls::{Field, Notice};
use firemage_webui_provider_api::{encode, get, text};

#[component]
pub fn NetworkFields(fields: Fields, vm_id: String) -> Element {
    let Fields {
        mut network,
        mut address,
        mut mac,
        ..
    } = fields;
    let networks = use_resource(|| async { get("/v1/networks").await });
    use_effect(move || {
        let reference = network();
        if let Some(Ok(rows)) = networks.read().as_ref()
            && let Some(rows) = rows.as_array()
            && !rows.iter().any(|row| text(row, "id") == reference)
            && let Some(row) = rows.iter().find(|row| text(row, "name") == reference)
        {
            network.set(text(row, "id"));
        }
    });
    let suggestion = use_resource(move || {
        let network = network();
        let vm_id = vm_id.clone();
        async move {
            if network.is_empty() {
                return Ok(None);
            }
            let query = if vm_id.is_empty() {
                String::new()
            } else {
                format!("?vm={}", encode(&vm_id))
            };
            get(&format!(
                "/v1/networks/{}/suggestion{query}",
                encode(&network)
            ))
            .await
            .map(Some)
        }
    });
    use_effect(move || {
        if let Some(Ok(Some(value))) = suggestion.read().as_ref() {
            if text(value, "network") != network() {
                return;
            }
            if address.peek().is_empty() {
                address.set(text(value, "address"));
            }
            if mac.peek().is_empty() {
                mac.set(text(value, "mac"));
            }
        }
    });
    rsx! {
        label { class: "field", r#for: "vm-network", span { "Network" }
            select { id: "vm-network", value: network(), onchange: move |event| {
                network.set(event.value()); address.set(String::new()); mac.set(String::new());
            },
                option { value: "", selected: network().is_empty(), "No network" }
                if let Some(Ok(rows)) = networks.read().as_ref() {
                    for row in rows.as_array().into_iter().flatten() {
                        option { value: text(row, "id"), selected: network() == text(row, "id"), "{text(row, \"name\")}" }
                    }
                }
            }
        }
        if !network().is_empty() {
            if let Some(Err(error)) = suggestion.read().as_ref() { Notice { message: error.clone() } }
            div { class: "form-grid",
                Field { label: "Guest IPv4 address", id: "vm-address", value: address, required: true }
                Field { label: "MAC address", id: "vm-mac", value: mac, required: true }
            }
            p { class: "small muted", "Available addresses are filled in from the selected network. You can change them. Jailed VMs with a network interface also need a guest address." }
        } else {
            p { class: "small muted", "No network interface or guest IP address will be configured." }
        }
    }
}
