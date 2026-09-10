//! Selected VM lifecycle controls and operational views.
mod advanced;
mod tabs;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use firemage_webui_view_vm_editor::VmEditor;
use serde_json::{Value, json};
#[component]
pub fn VmDetail(vm: Value, onchanged: EventHandler<()>, onclose: EventHandler<()>) -> Element {
    let auth = use_auth();
    let id = text(&vm, "id");
    let state = text(&vm, "state");
    let mut tab = use_signal(|| "Overview".to_owned());
    let mut editing = use_signal(|| false);
    let mut confirm = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let action_id = id.clone();
    let mut action = move |name: String| {
        let id = action_id.clone();
        busy.set(true);
        error.set(String::new());
        spawn(async move {
            let result = if name == "delete" {
                request("DELETE", &format!("/v1/vms/{id}"), None, &auth.csrf()).await
            } else {
                request(
                    "POST",
                    &format!("/v1/vms/{id}/actions"),
                    Some(json!({ "action" : name })),
                    &auth.csrf(),
                )
                .await
            };
            match result {
                Ok(_) => {
                    onchanged.call(());
                    if name == "delete" {
                        onclose.call(());
                    }
                }
                Err(e) => {
                    error.set(e);
                    onchanged.call(());
                }
            }
            busy.set(false);
        });
    };
    rsx! {
        aside { class: "vm-detail",
            div { class: "heading compact",
                div {
                    h2 { r#"{vm["spec"]["name"].as_str().unwrap_or_default()}"# }
                    span { class: "mono small muted", "{id}" }
                }
                button {
                    class: "icon-button",
                    "aria-label": "Close VM details",
                    onclick: move |_| onclose.call(()),
                    Icon { name: "close" }
                }
            }
            Status { value: state.clone() }
            Notice { message: error() }
            if let Some(persisted) = vm["error"].as_str() {
                if !error().starts_with(persisted) {
                    Notice { message: persisted.to_owned() }
                }
            }
            if auth.is_admin() {
                div { class: "actions wrap",
                    for (label, command, enabled) in [
                        (
                            "Start",
                            "start",
                            matches!(state.as_str(), "defined" | "ready" | "stopped" | "failed"),
                        ),
                        (
                            "Prepare",
                            "prepare",
                            matches!(state.as_str(), "defined" | "stopped" | "failed"),
                        ),
                        ("Launch", "launch", state == "ready"),
                        ("Pause", "pause", state == "running"),
                        ("Resume", "resume", state == "paused"),
                        ("Shut down", "shutdown", state == "running"),
                        (
                            "Stop",
                            "stop",
                            matches!(state.as_str(), "running" | "paused" | "starting" | "unknown"),
                        ),
                        ("Refresh", "refresh", true),
                    ]
                    {
                        if enabled {
                            button {
                                disabled: busy(),
                                class: if command == "start" { "primary" } else if command == "stop" { "danger subtle" } else { "" },
                                onclick: {
                                    let mut action = action.clone();
                                    move |_| {
                                        if command == "stop" {
                                            confirm.set("stop".into());
                                        } else {
                                            action(command.into());
                                        }
                                    }
                                },
                                "{label}"
                            }
                        }
                    }
                }
            }
            div { class: "tabs scroll",
                for label in ["Overview", "Security", "Egress", "Environment", "Boot", "Logs", "Files", "Metadata", "Advanced"] {
                    button {
                        class: if tab() == label { "active" } else { "" },
                        onclick: move | _
                                                        | tab.set(label.into()),
                        "{label}"
                    }
                }
            }
            match tab().as_str() {
                "Overview" => rsx! {
                    dl { class: "key-values",
                        dt { "vCPUs" }
                        dd { r#"{vm["spec"]["vcpus"]}"# }
                        dt { "Memory" }
                        dd { r#"{vm["spec"]["memory_mib"]} MiB"# }
                        dt { "Root disk" }
                        dd { r#"{vm["spec"]["rootfs"]["kind"].as_str().unwrap_or("External")}"# }
                        dt { "Network" }
                        dd { r#"{vm["spec"]["network"]["network"].as_str().unwrap_or("No network")}"# }
                        dt { "Guest address" }
                        dd { r#"{vm["spec"]["network"]["address"].as_str().unwrap_or("None")}"# }
                        dt { "Boot files" }
                        dd { r#"{vm["spec"]["files"].as_array().map_or(0,Vec::len)}"# }
                        dt { "Userdata" }
                        dd {
                            if vm["spec"]["userdata"].is_string() {
                                "Provided"
                            } else {
                                "None"
                            }
                        }
                        dt { "Owner" }
                        dd { class: "mono small", r#"{vm["owner_id"].as_str().unwrap_or_default()}"# }
                    }
                    if auth.is_admin() {
                        div { class: "actions wrap",
                            button {
                                disabled: ! matches!(state.as_str(), "defined" |
                                                                        "stopped" | "failed"),
                                onclick: move |_| editing.set(true),
                                "Configure VM"
                            }
                            button {
                                class: "danger subtle",
                                disabled: busy() || matches!(state.as_str(), "running" | "paused" | "starting"),
                                onclick: move | _ | confirm.set("delete"
                                                                        .into()),
                                "Delete VM"
                            }
                        }
                        p { class: "small muted", "Stop the VM before changing its configuration or deleting it." }
                    }
                },
                "Security" => rsx! { firemage_webui_view_security::Security { vm: vm.clone(), onchanged } },
                "Boot" => rsx! { firemage_webui_view_boot::Boot { vm: vm.clone(), onchanged } },
                "Environment" => rsx! { firemage_webui_view_environment::Environment { vm: vm.clone(), onchanged } },
                "Egress" => rsx! {
                    firemage_webui_view_egress::Egress { vm: vm.clone(), onchanged }
                },
                "Logs" => rsx! {
                    tabs::Logs { id: id.clone() }
                },
                "Files" => rsx! {
                    tabs::Files { id: id
                                                        .clone() }
                },
                "Metadata" => rsx! {
                    tabs::Metadata { id: id.clone(), initial: vm["spec"]["metadata"].clone(), onchanged }
                },
                _ => rsx! {
                    advanced::Advanced { id: id.clone(), onchanged }
                },
            }
            if editing() {
                VmEditor {
                    vm: vm.clone(),
                    onclose: move |_| editing.set(false),
                    onsaved: move |_| {
                        editing.set(false);
                        onchanged.call(());
                    },
                }
            }
            if !confirm().is_empty() {
                Confirm {
                    title: if confirm() == "delete" { "Delete virtual machine?" } else { "Stop virtual machine?" },
                    description: if confirm() == "delete" { "This removes the VM definition and its managed files. Download any outputs you need first." } else { "This immediately stops the process. Guest applications may not finish writing their output. Use Shut down for a graceful guest shutdown." },
                    label: if confirm() == "delete" { "Delete VM" } else { "Stop VM" },
                    onclose: move |_| confirm.set(String::new()),
                    onconfirm: move |_| {
                        action(confirm());
                        confirm.set(String::new());
                    },
                }
            }
        }
    }
}
