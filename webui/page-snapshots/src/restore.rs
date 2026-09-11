use dioxus::prelude::*;
use firemage_webui_component_controls::{Confirm, Notice};
use firemage_webui_provider_api::{encode, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn Restore(
    snapshot: Value,
    #[props(default)] vm: Option<Value>,
    onchanged: EventHandler<()>,
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
    let mut target_name = use_signal(String::new);
    let mut acknowledged = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let mut success = use_signal(|| false);
    let mut pending = use_signal(|| None::<Value>);
    let choices: Vec<Value> = rows
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|rows| {
            rows.iter()
                .filter(|vm| crate::model::is_restore_target(vm, &snapshot))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let chosen = if fixed {
        choices.first().cloned()
    } else {
        choices
            .iter()
            .find(|vm| crate::model::vm_choice(vm) == target_name())
            .cloned()
    };
    let list_id = format!("snapshot-targets-{}", text(&snapshot, "id"));
    let trusted = snapshot["trusted"] == true;
    rsx! {
        section { class: "snapshot-restore",
            if fixed { crate::requirements::Requirements { snapshot: snapshot.clone() } }
            h3 { "Restore to VM" }
            if let Some(Err(message)) = rows.read().as_ref() { Notice { message: message.clone() } }
            if !fixed {
                label { class: "field", span { "Target VM" }
                    input { list: list_id.clone(), value: r#"{target_name}"#, placeholder: "Search compatible VMs", disabled: busy(), oninput: move |event| target_name.set(event.value()) }
                }
                datalist { id: list_id,
                    for vm in &choices { option { value: crate::model::vm_choice(vm), r#"{text(vm, "state")} · {text(vm, "owner_id")}"# } }
                }
            }
            if !trusted {
                label { class: "snapshot-trust switch-row",
                    span { "I trust the source of this snapshot." }
                    input { class: "toggle-switch", r#type: "checkbox", role: "switch", checked: acknowledged(), disabled: busy(), onchange: move |event| acknowledged.set(event.checked()) }
                }
            }
            p { class: "muted small", "Restoring replaces the target VM's disks and leaves it paused. The saved source name is informational." }
            if choices.is_empty() && rows.read().is_some() { p { class: "muted small", "No stopped compatible jailed VM is available. CPU and memory must match." } }
            Notice { message: error(), success: success() }
            button { class: "primary", disabled: !auth.is_admin() || busy() || chosen.is_none() || (!trusted && !acknowledged()),
                onclick: move |_| {
                    if !auth.is_admin() || busy() || (!trusted && !acknowledged()) { return; }
                    pending.set(chosen.clone());
                }, if busy() { "Restoring…" } else { "Restore snapshot" }
            }
            if let Some(target) = pending() {
                Confirm {
                    title: "Restore snapshot?",
                    description: format!("Replace the managed disks of {} with this snapshot? Existing disk contents will be lost. The VM will be left paused.", crate::model::vm_choice(&target)),
                    label: "Restore snapshot",
                    onclose: move |_| pending.set(None),
                    onconfirm: move |_| {
                        if !auth.is_admin() || busy() || (!trusted && !acknowledged()) { return; }
                        let Some(target) = pending.write().take() else { return; };
                        let snapshot_id = text(&snapshot, "id"); let vm_id = text(&target, "id");
                        busy.set(true); error.set(String::new()); success.set(false);
                        spawn(async move {
                            let result = async {
                                if !trusted { request("POST", &format!("/v1/snapshots/{}/trust", encode(&snapshot_id)), None, &auth.csrf()).await?; }
                                request("POST", &format!("/v1/vms/{}/snapshots/restore", encode(&vm_id)), Some(json!({"snapshot_id":snapshot_id})), &auth.csrf()).await
                            }.await;
                            match result {
                                Ok(_) => { success.set(true); error.set("Snapshot restored. The VM is paused; resume it when ready.".into()); onchanged.call(()); },
                                Err(message) => error.set(message),
                            }
                            busy.set(false);
                        });
                    },
                }
            }
        }
    }
}
