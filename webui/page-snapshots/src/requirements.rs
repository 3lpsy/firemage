use dioxus::prelude::*;
use firemage_webui_provider_api::text;
use serde_json::Value;

#[component]
pub fn Requirements(snapshot: Value) -> Element {
    let requirements = &snapshot["requirements"];
    let network = &requirements["network"];
    let definition = &requirements["network_definition"];
    let drives = requirements["drives"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    rsx! {
        section { class: "snapshot-requirements",
            h3 { "Restore requirements" }
            dl { class: "snapshot-facts",
                dt { "vCPUs" } dd { r#"{snapshot["vcpus"]}"# }
                dt { "Memory" } dd { r#"{snapshot["memory_mib"]} MiB"# }
                dt { "Saved network" } dd { if network.is_null() { "None" } else { r#"{snapshot["network_definition"]["name"].as_str().unwrap_or_else(|| network["network"].as_str().unwrap_or_default())}"# } }
                if !definition.is_null() {
                    dt { "Subnet" } dd { class: "mono", r#"{text(definition, "subnet")}"# }
                    dt { "Gateway" } dd { class: "mono", r#"{text(definition, "gateway")}"# }
                    dt { "Network policy" } dd { r#"{text(&definition["policy"], "mode")}"#
                        if let Some(address) = definition["policy"]["address"].as_str() { " · {address}" }
                    }
                }
                if !network.is_null() {
                    dt { "Guest IPv4" } dd { class: "mono", r#"{text(network, "address")}"# }
                    dt { "Guest MAC" } dd { class: "mono", r#"{text(network, "mac")}"# }
                }
                dt { "Initrd" } dd { if requirements["initrd"] == true { "Required" } else { "None" } }
                dt { "Additional drives" }
                dd {
                    if drives.is_empty() { "None" }
                    for drive in &drives {
                        div { class: "snapshot-drive", strong { r#"{text(drive, "id")}"# } " · "
                            if drive["read_only"] == true { "Read-only" } else { "Writable" }
                        }
                    }
                }
            }
            p { class: "muted small", "The target must match these settings. An equivalent network may use another name; guest IP and MAC stay the same. Restore replaces the target's disks." }
        }
    }
}
