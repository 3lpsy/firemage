use dioxus::prelude::*;
use firemage_webui_component_controls::{Field, Modal, Notice};
use firemage_webui_provider_api::{encode, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn Save(
    #[props(default)] vm: Option<Value>,
    onclose: EventHandler<()>,
    onsaved: EventHandler<String>,
) -> Element {
    let auth = use_auth();
    let fixed = vm.is_some();
    let rows = use_resource(move || {
        let vm = vm.clone();
        async move {
            match vm {
                Some(vm) => Ok(vec![vm]),
                None => crate::model::vms().await,
            }
        }
    });
    let mut target = use_signal(String::new);
    let alias = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let choices: Vec<Value> = rows
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|rows| {
            rows.iter()
                .filter(|vm| crate::model::is_capture_target(vm))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let chosen = if fixed {
        choices.first().cloned()
    } else {
        choices
            .iter()
            .find(|vm| crate::model::vm_choice(vm) == target())
            .cloned()
    };
    let can_save = chosen.is_some();
    rsx! {
        Modal { title: "Save snapshot", onclose: move |_| { if !busy() { onclose.call(()); } },
            form { onsubmit: move |event| {
                event.prevent_default();
                if busy() || !auth.is_admin() { return; }
                let Some(vm) = chosen.clone() else { return; };
                let alias = alias();
                if let Err(message) = firemage_wire::ensure_asset_alias(&alias) { error.set(message.to_string()); return; }
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request("POST", &format!("/v1/vms/{}/snapshots", encode(&text(&vm, "id"))), Some(json!({"alias":alias})), &auth.csrf()).await {
                        Ok(snapshot) => onsaved.call(text(&snapshot, "id")), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                if !fixed {
                    label { class: "field", span { "Paused VM" }
                        input { list: "snapshot-save-vms", value: r#"{target}"#, placeholder: "Search paused VMs", disabled: busy(), oninput: move |event| target.set(event.value()) }
                    }
                    datalist { id: "snapshot-save-vms", for vm in &choices { option { value: crate::model::vm_choice(vm), r#"{text(vm, "owner_id")}"# } } }
                }
                Field { id: "snapshot-save-alias", label: "Alias", value: alias, required: true, disabled: busy(), placeholder: "review-checkpoint" }
                if let Some(Err(message)) = rows.read().as_ref() { Notice { message: message.clone() } }
                if choices.is_empty() && rows.read().is_some() { p { class: "muted small", "Pause a jailed VM before saving a snapshot." } }
                p { class: "muted small", "Saves guest state, memory and disks. The VM stays paused." }
                Notice { message: error() }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy() || !can_save || !auth.is_admin(), if busy() { "Saving…" } else { "Save snapshot" } }
                }
            }
        }
    }
}
