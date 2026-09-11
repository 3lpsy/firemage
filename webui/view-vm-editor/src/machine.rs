use crate::fields::Fields;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;

#[component]
pub fn Machine(fields: Fields, existing: bool) -> Element {
    let Fields {
        name,
        vcpus,
        memory,
        mut mode,
        isolation,
        socket,
        mut terminal,
        ..
    } = fields;
    rsx! {
        div { class: "vm-form-machine",
            Field { label: "Name", id: "vm-name", value: name, required: true, placeholder: "build-runner" }
            Field { label: "vCPUs", id: "vm-cpus", value: vcpus, kind: "number", required: true }
            Field { label: "Memory (MiB)", id: "vm-memory", value: memory, kind: "number", required: true }
        }
        fieldset { disabled: existing, legend { "Runtime" },
            div { class: "radio-group",
                for (value, label) in [("managed", "Managed by Firemage"), ("socket", "Bring your own socket")] {
                    label { input { r#type: "radio", name: "runtime-mode", value, checked: mode() == value,
                        onchange: move |_| { mode.set(value.into()); if value == "socket" { terminal.set(false); } } } "{label}" }
                }
            }
        }
        if mode() == "socket" {
            Field { label: "Firecracker socket on host", id: "vm-socket", value: socket, disabled: existing, required: true }
            p { class: "small muted", "External process mode requires server opt-in. The process owner manages isolation and limits." }
        } else if !existing {
            firemage_webui_view_security::IsolationMode { value: isolation }
        }
        if existing { p { class: "small muted", "Runtime socket and isolation mode were fixed at creation." } }
    }
}
