//! Selected VM lifecycle controls and operational views.
mod advanced;
mod attachments;
mod duplicate;
mod lifecycle;
mod tabs;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
#[component]
pub fn VmDetail(
    vm: Value,
    onchanged: EventHandler<()>,
    onclose: EventHandler<()>,
    #[props(default)] full_page: bool,
) -> Element {
    let auth = use_auth();
    let networks =
        use_resource(|| async { firemage_webui_provider_api::get("/v1/networks").await });
    let id = text(&vm, "id");
    let state = text(&vm, "state");
    let mut tab = use_signal(|| "Overview".to_owned());
    let mut duplicating = use_signal(|| false);
    let edit_id = id.clone();
    let edit =
        EventHandler::new(move |_: ()| firemage_webui_routes::navigate_vm_editor(Some(&edit_id)));
    let mut confirm = use_signal(String::new);
    let mut delete_snapshots = use_signal(|| false);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let action_id = id.clone();
    let mut action = move |name: String| {
        let id = action_id.clone();
        busy.set(true);
        error.set(String::new());
        spawn(async move {
            let result = if name == "delete" {
                request(
                    "DELETE",
                    &format!("/v1/vms/{id}?delete_snapshots={}", delete_snapshots()),
                    None,
                    &auth.csrf(),
                )
                .await
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
        aside { id: "vm-detail", class: if full_page { "vm-detail vm-full-page" } else { "vm-detail" },
            div { class: "heading compact",
                div {
                    h2 { r#"{vm["spec"]["name"].as_str().unwrap_or_default()}"# }
                    if full_page { Status { value: state.clone() } }
                    span { class: "mono small muted", "{id}" }
                }
                if !full_page {
                    a { class: "button", href: "#vms/{id}", "Open full page" }
                }
                button {
                    class: "icon-button",
                    "aria-label": "Close VM details",
                    onclick: move |_| onclose.call(()),
                    Icon { name: "close" }
                }
            }
            if !full_page { Status { value: state.clone() } }
            if full_page {
                div { class: "vm-page-summary",
                    span { strong { r#"{vm["spec"]["vcpus"]}"# } " vCPUs" }
                    span { strong { r#"{vm["spec"]["memory_mib"]} MiB"# } " memory" }
                    span { r#"{vm["spec"]["network"]["network"].as_str().unwrap_or("No network")}"# }
                    if let Some(address) = vm["spec"]["network"]["address"].as_str() { span { "{address}" } }
                    span { r#"{vm["spec"]["security"]["mode"].as_str().unwrap_or("jailed")}"# }
                }
            }
            Notice { message: error() }
            if let Some(persisted) = vm["error"].as_str() {
                if !error().starts_with(persisted) {
                    Notice { message: persisted.to_owned() }
                }
            }
            if auth.is_admin() {
                div { class: "actions wrap",
                    for (label, command, enabled) in lifecycle::controls(&vm, &networks.read().as_ref().and_then(|value| value.as_ref().ok()).cloned().unwrap_or_default())
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
                    button { disabled: busy() || vm["spec"]["socket"].is_string(), onclick: move |_| duplicating.set(true), "Duplicate VM" }
                    button {
                        class: "danger subtle",
                        disabled: busy() || !matches!(state.as_str(), "defined" | "stopped" | "failed"),
                        title: "Stop the VM before deleting its managed disks and files",
                        onclick: move |_| { delete_snapshots.set(false); confirm.set("delete".into()); },
                        "Delete VM"
                    }
                }
            }
            div { class: "tabs scroll",
                for label in ["Overview", "Configuration", "Security", "Egress", "Environment", "Attachments", "Boot", "Serial", "Firecracker", "Files", "Snapshots", "Metadata", "Advanced"] {
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
                                onclick: move |_| edit.call(()),
                                "Configure VM"
                            }

                        }
                        p { class: "small muted", "Stop the VM before changing its configuration or deleting it." }
                    }
                },
                "Configuration" => rsx! { firemage_webui_view_vm_transfer::Configuration { vm: vm.clone(), onedit: edit, onchanged } },
                "Security" => rsx! { firemage_webui_view_security::Security { vm: vm.clone(), onchanged } },
                "Attachments" => rsx! { attachments::Attachments { vm: vm.clone(), onedit: edit } },
                "Boot" => rsx! { firemage_webui_view_boot::Boot { vm: vm.clone(), onchanged } },
                "Environment" => rsx! { firemage_webui_view_environment::Environment { vm: vm.clone(), onchanged } },
                "Egress" => rsx! {
                    firemage_webui_view_egress::Egress { vm: vm.clone(), onchanged }
                },
                "Serial" => rsx! {
                    firemage_webui_view_serial::Serial { vm: vm.clone(), onchanged }
                },
                "Firecracker" => rsx! { firemage_webui_view_serial::FirecrackerLogs { id: id.clone() } },
                "Files" => rsx! {
                    firemage_webui_view_guest_files::GuestFiles { vm: vm.clone() }
                },
                "Snapshots" => rsx! { firemage_webui_page_snapshots::VmSnapshots { vm: vm.clone(), onchanged } },
                "Metadata" => rsx! {
                    tabs::Metadata { id: id.clone(), initial: vm["spec"]["metadata"].clone(), onchanged }
                },
                _ => rsx! {
                    advanced::Advanced { id: id.clone(), onchanged }
                },
            }
            if duplicating() {
                duplicate::DuplicateVm { vm: vm.clone(), onclose: move |_| duplicating.set(false), onsaved: move |new_id: String| {
                    duplicating.set(false); onchanged.call(());
                    firemage_webui_routes::navigate_vm(&new_id);
                } }
            }
            if confirm() == "delete" {
                Modal { title: "Delete virtual machine?", onclose: move |_| confirm.set(String::new()),
                    p { "This deletes the VM and its managed disks. Download any output you need first." }
                    label { class: "checkbox-row",
                        input { r#type: "checkbox", checked: delete_snapshots(), onchange: move |event| delete_snapshots.set(event.checked()) }
                        "Also delete snapshots taken from this VM"
                    }
                    p { class: "small muted", "Snapshots are kept in the snapshot library unless selected above. Shared kernels, file assets and secrets are kept." }
                    div { class: "actions end",
                        button { onclick: move |_| confirm.set(String::new()), "Cancel" }
                        button { class: "danger", onclick: move |_| { action("delete".into()); confirm.set(String::new()); }, "Delete VM" }
                    }
                }
            } else if confirm() == "stop" {
                Confirm {
                    title: "Stop virtual machine?",
                    description: "This immediately stops the process. Guest applications may not finish writing their output. Use Shut down for a graceful guest shutdown.",
                    label: "Stop VM",
                    onclose: move |_| confirm.set(String::new()),
                    onconfirm: move |_| { action("stop".into()); confirm.set(String::new()); },
                }
            }
        }
    }
}
