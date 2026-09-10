//! Per-VM egress policy, connection instructions, and editor.
mod editor;
mod fields;
mod injection;
mod model;
mod summary;
mod upstream;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{get, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Egress(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let id = text(&vm, "id");
    let mut refresh = use_signal(|| 0u32);
    let mut editing = use_signal(|| false);
    let status = use_resource(move || {
        let id = id.clone();
        let _ = refresh();
        async move { get(&format!("/v1/vms/{id}/egress")).await }
    });
    let can_edit = auth.is_admin()
        && matches!(
            text(&vm, "state").as_str(),
            "defined" | "stopped" | "failed"
        );
    rsx! {
        div { class: "heading compact",
            h3 { "Egress"
                Info { title: "Controlled guest connectivity",
                    "Use a Firemage-only network to permit only the VM’s assigned proxy and tunnel ports on the gateway. Direct upstream access, other host ports, and other guests remain blocked. HTTP rules apply to this VM only."
                }
            }
            button { onclick: move |_| refresh += 1, "Refresh egress" }
        }
        match status.read().as_ref() {
            Some(Ok(value)) => rsx! { summary::Summary { value: value.clone() } },
            Some(Err(error)) => rsx! { Notice { message: error.clone() } },
            None => rsx! { p { class: "muted", "Loading egress…" } },
        }
        if auth.is_admin() {
            button { class: "primary", disabled: !can_edit, onclick: move |_| editing.set(true), "Configure egress" }
            p { class: "small muted", "Stop the VM to edit destinations. Attach a Firemage-only network before saving." }
        }
        if editing() {
            editor::EgressEditor { vm: vm.clone(), onclose: move |_| editing.set(false),
                onsaved: move |_| { editing.set(false); refresh += 1; onchanged.call(()); }
            }
        }
    }
}
