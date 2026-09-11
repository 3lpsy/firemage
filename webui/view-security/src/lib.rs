//! Host isolation and process limits in the VM inspector.
mod editor;
pub use editor::LimitsEditor;
mod mode;
mod model;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_auth::use_auth;
pub use mode::IsolationMode;
use serde_json::Value;

#[component]
pub fn Security(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut editing = use_signal(|| false);
    let mode = model::mode(&vm["spec"]);
    let can_edit = matches!(vm["state"].as_str(), Some("defined" | "stopped" | "failed"));
    let limits = model::LimitsDraft::from_spec(&vm["spec"]);
    let memory_ceiling = vm["spec"]["memory_mib"].as_u64().unwrap_or(256)
        + limits.0[0].parse::<u64>().unwrap_or(256);
    rsx! {
        h3 { "Host isolation"
            Info { title: "VM security boundaries",
                p { "The isolation mode controls the Firecracker process on the host. A jailed VM uses a private filesystem, non-root host identity, seccomp, and cgroup limits. Guest root does not grant host root." }
                p { "Network access, proxy destinations, guest credentials, and boot file permissions are configured separately. Firemage copies boot assets and never mounts a host directory into the guest." }
                p { "The mode is fixed when the VM is created. Create a new VM to change it. Trusted and external modes require explicit server operator opt-in." }
            }
        }
        dl { class: "key-values",
            dt { "Mode" } dd { "{model::label(mode)}" }
            if mode == "jailed" {
                dt { "Host identity" } dd { "Dedicated non-root UID/GID" }
                dt { "Filesystem" } dd { "Private jail" }
                dt { "System calls" } dd { "Firecracker seccomp filter" }
                dt { "Host memory ceiling" }
                dd { "{memory_ceiling} MiB" }
                for (index, (_, label, placeholder)) in model::LIMITS.into_iter().enumerate() {
                    dt { "{label}" }
                    dd { if limits.0[index].is_empty() { "{placeholder}" } else { "{limits.0[index]}" } }
                }
            }
        }
        if mode == "trusted" {
            p { class: "small muted", "Firecracker runs with the service's host permissions. Jailer isolation and managed host limits are not applied." }
        } else if mode == "external" {
            p { class: "small muted", "The external process owner is responsible for host isolation, limits, and lifecycle. Firemage cannot verify or apply those protections." }
        }
        if auth.is_admin() && mode == "jailed" {
            button { class: "primary egress-section", disabled: !can_edit, onclick: move |_| editing.set(true), "Configure host limits" }
            p { class: "small muted", "Stop the VM before changing host limits. Mode changes require a new VM." }
        }
        if editing() {
            editor::LimitsEditor { vm: vm.clone(), onclose: move |_| editing.set(false), onsaved: move |_| { editing.set(false); onchanged.call(()); } }
        }
    }
}
