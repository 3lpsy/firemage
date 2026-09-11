use dioxus::prelude::*;
use serde_json::{Value, json};

#[component]
pub fn Subeditors(
    kind: String,
    spec: Value,
    onclose: EventHandler<()>,
    onapply: EventHandler<Value>,
) -> Element {
    let vm = json!({"spec":spec});
    match kind.as_str() {
        "egress" => {
            rsx! { firemage_webui_view_egress::EgressEditor { vm, onclose, onapply, onsaved: |_| {} } }
        }
        "environment" => {
            rsx! { firemage_webui_view_environment::EnvironmentEditor { vm, onclose, onapply, onsaved: |_| {} } }
        }
        "security" => {
            rsx! { firemage_webui_view_security::LimitsEditor { vm, onclose, onapply, onsaved: |_| {} } }
        }
        "boot" => {
            rsx! { firemage_webui_view_boot::BootEditor { vm, onclose, onapply, onsaved: |_| {} } }
        }
        "storage" => {
            rsx! { crate::extra_editor::StorageEditor { spec: vm["spec"].clone(), onclose, onapply } }
        }
        _ => rsx! {},
    }
}
