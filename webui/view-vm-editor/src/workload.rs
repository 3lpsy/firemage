use crate::fields::Fields;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;

#[component]
pub fn Workload(fields: Fields) -> Element {
    let Fields {
        mut workload_mode,
        command,
        ..
    } = fields;
    rsx! {
        if (fields.mode)() == "managed" && (fields.source)() == "oci" {
            fieldset { legend { "After the workload exits" },
                div { class: "radio-group",
                    for (value, label) in [("one-shot", "One-shot"), ("keep-alive", "Keep alive")] {
                        label { input { r#type: "radio", name: "workload-mode", value, checked: workload_mode() == value,
                            onchange: move |_| workload_mode.set(value.into()) } "{label}" }
                    }
                }
            }
            p { class: "small muted", "One-shot shuts down the guest when the image command exits. Keep alive leaves the guest running." }
            Editor { label: "Command arguments (JSON array, optional)", id: "vm-workload-command", value: command, rows: 4 }
            p { class: "small muted", r#"Leave blank to use the image command. For a shell script use ["/bin/sh", "-lc", "your script"]."# }
        } else {
            p { class: "small muted", "Workload controls require a managed OCI image. Other guests manage their own startup and shutdown." }
        }
    }
}
