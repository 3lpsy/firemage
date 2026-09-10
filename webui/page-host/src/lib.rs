//! Live host capacity and runtime prerequisites.
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::get;
#[component]
pub fn Host() -> Element {
    let mut refresh = use_signal(|| 0);
    let host = use_resource(move || {
        let _ = refresh();
        async { get("/v1/host").await }
    });
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "OBSERVABILITY" }
                h1 { "Host" }
                p { class: "muted", "Compute capacity and Firecracker runtime availability." }
            }
            button { onclick: move |_| refresh += 1, "Refresh" }
        }
        match host.read().as_ref() {
            Some(Ok(v)) => rsx! {
                div { class: "metrics",
                    Metric { label: "Host CPUs", value: v["cpus"].to_string() }
                    Metric {
                        label: "Total memory",
                        value: format!("{:.1}", v["memory_total_bytes"].as_u64().unwrap_or(0) as f64 / 1073741824.),
                        unit: "GiB",
                    }
                    Metric {
                        label: "Available memory",
                        value: format!(
                            "{:.1}",
                            v["memory_available_bytes"].as_u64().unwrap_or(0) as f64 / 1073741824.,
                        ),
                        unit: "GiB",
                    }
                    Metric { label: "Running VMs", value: v["vms"]["running"].to_string() }
                }
                section { class: "host-panel",
                    h2 { "Runtime" }
                    dl { class: "key-values",
                        dt { "Firemage" }
                        dd { r#"{v["version"].as_str().unwrap_or_default()}"# }
                        dt { "KVM" }
                        dd {
                            Status { value: (if v["kvm_available"] == true { "available" } else { "unavailable" }).to_owned() }
                        }
                        dt { "Uptime" }
                        dd { r#"{v["uptime_seconds"].as_u64().unwrap_or(0)/60} minutes"# }
                        dt { "Virtual machines" }
                        dd { r#"{v["vms"]["total"]}"# }
                        dt { "Networks" }
                        dd { r#"{v["networks"]}"# }
                        dt { "Users" }
                        dd { r#"{v["users"]}"# }
                    }
                    p { class: "muted small",
                        "Host availability does not guarantee that every guest image can boot. Inspect VM logs for guest-specific failures."
                    }
                }
            },
            Some(Err(e)) => rsx! {
                Notice { message: e.clone() }
            },
            None => rsx! {
                p { class: "muted", "Loading host…" }
            },
        }
    }
}
