use crate::model::{LIMITS, LimitsDraft};
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn LimitsEditor(
    vm: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
    #[props(default)] onapply: Option<EventHandler<Value>>,
) -> Element {
    let auth = use_auth();
    let mut draft = use_signal(|| LimitsDraft::from_spec(&vm["spec"]));
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let can_save = crate::model::is_editable(&vm, onapply.is_some());
    rsx! {
        Modal { title: "Configure host limits", onclose,
            form { class: "modal-form", onsubmit: move |event| {
                event.prevent_default();
                if busy() || !can_save { return; }
                let spec = match draft.read().apply(&vm["spec"]) {
                    Ok(spec) => spec,
                    Err(message) => { error.set(message); return; }
                };
                if let Some(onapply) = onapply { onapply.call(spec); return; }
                let path = format!("/v1/vms/{}", text(&vm, "id"));
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request("PUT", &path, Some(spec), &auth.csrf()).await {
                        Ok(_) => onsaved.call(()), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    p { class: "small muted", "These limits apply to the host Firecracker process. Guest memory and vCPUs remain in the VM configuration." }
                    for (index, (key, label, placeholder)) in LIMITS.into_iter().enumerate() {
                        label { class: "field", r#for: "security-{key}", span { "{label}"
                            if key == "cpu_percent" { Info { title: "Host CPU quota", "Leave CPU quota blank for 100% per configured vCPU. Automatic file size covers writable disks and guest memory plus 64 MiB for snapshots. These limits apply to the Firecracker process on the host." } }
                        }
                            input { id: "security-{key}", disabled: !can_save, r#type: "number", min: "1", step: "1", required: !matches!(index, 2 | 3),
                                value: "{draft.read().0[index]}", placeholder,
                                oninput: move |event| draft.write().0[index] = event.value(),
                            }
                        }
                    }
                }
                if onapply.is_some() { p { class: "small muted", "Applied changes are saved when you submit the VM form." } }
                if !can_save { Notice { message: "Stop the VM before saving host limits. Updated limits apply the next time it starts. Changing the isolation mode requires a new VM." } }
                div { class: "actions end",
                    button { r#type: "button", onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy() || !can_save, if onapply.is_some() { "Apply to draft" } else { "Save host limits" } }
                }
            }
        }
    }
}
