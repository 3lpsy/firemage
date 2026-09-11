use crate::{fields::Fields, sections::Section};
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use serde_json::Value;

#[component]
pub fn Guided(
    fields: Fields,
    base: Signal<Value>,
    existing: bool,
    vm_id: String,
    owner: String,
    onconfigure: EventHandler<String>,
) -> Element {
    let mut terminal = fields.terminal;
    let attachments = fields.attachments;
    let count = |key: &str| base.read()[key].as_array().map_or(0, Vec::len);
    let environment_count = base.read()["environment"]
        .as_object()
        .map_or(0, serde_json::Map::len);
    let egress_summary = if base.read()["egress_policy"].is_string() {
        "Shared policy"
    } else {
        "No outbound access"
    };
    rsx! {
        div { class: "vm-form-layout",
            crate::sections::SectionIndex {}
            div { class: "vm-form-sections",
                Section { id: "machine", title: "Machine", summary: format!("{} · {}", (fields.mode)(), (fields.isolation)()), expanded: true,
                    crate::machine::Machine { fields, existing }
                }
                Section { id: "storage", title: "Image and disks", summary: format!("{} · {} additional drives", (fields.source)(), count("drives")), expanded: true,
                    crate::storage::Storage { fields, existing, onconfigure }
                }
                Section { id: "workload", title: "Workload", summary: if (fields.source)() == "oci" { (fields.workload_mode)() } else { "Guest managed".into() },
                    crate::workload::Workload { fields }
                }
                Section { id: "network", title: "Network", summary: if (fields.network)().is_empty() { "No network".into() } else { format!("{} · {}", (fields.network)(), (fields.address)()) },
                    crate::network::NetworkFields { fields, vm_id }
                }
                Section { id: "egress", title: "Egress", summary: egress_summary,
                    crate::egress::EgressFields { fields, base, owner, oncreate: move |_| onconfigure.call("egress".into()) }
                }
                Section { id: "environment", title: "Environment", summary: format!("{environment_count} variables"),
                    p { class: "small muted", "Set plain values or references to named secrets." }
                    button { r#type: "button", onclick: move |_| onconfigure.call("environment".into()), "Configure environment" }
                }
                Section { id: "attachments", title: "Attachments", summary: format!("{} files and secrets", attachments.read().len()), expanded: true,
                    firemage_webui_view_asset_attachments::Attachments { value: attachments }
                }
                Section { id: "boot", title: "Boot inputs", summary: format!("{} inline files{}", count("files"), if (fields.userdata)().is_empty() { "" } else { " · Userdata configured" }),
                    Field { label: "Boot arguments", id: "vm-boot-args", value: fields.boot_args }
                    Editor { label: "Userdata", id: "vm-userdata", value: fields.userdata, rows: 6 }
                    button { r#type: "button", onclick: move |_| onconfigure.call("boot".into()), "Configure boot inputs" }
                }
                Section { id: "security", title: "Security limits", summary: (fields.isolation)(),
                    if (fields.isolation)() == "jailed" && (fields.mode)() == "managed" {
                        p { class: "small muted", "Limit the host Firecracker process's memory overhead, CPU, threads and files." }
                        button { r#type: "button", onclick: move |_| onconfigure.call("security".into()), "Configure host limits" }
                    } else { p { class: "small muted", "Host limits are managed by Firemage for jailed VMs only." } }
                }
                Section { id: "metadata", title: "Metadata", summary: if (fields.metadata)().trim().is_empty() { "Not configured" } else { "Shared policy" },
                    Editor { label: "Metadata JSON (optional)", id: "vm-metadata", value: fields.metadata, rows: 8 }
                    p { class: "small muted", "MMDS v2 requires a network interface. Leave blank to disable metadata." }
                }
                Section { id: "terminal", title: "Terminal", summary: if terminal() { "Enabled" } else { "Disabled" },
                    label { class: "check-row", input { id: "vm-terminal", r#type: "checkbox", checked: terminal(), disabled: (fields.mode)() == "socket",
                        onchange: move |event| terminal.set(event.checked()) } "Enable guest serial input" }
                    p { class: "small muted", "A managed guest must provide a console on ttyS0." }
                }
            }
        }
    }
}
