//! Guided asset, capacity, and network fields.
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::get;
#[derive(Clone, Copy, PartialEq)]
pub struct Fields {
    pub name: Signal<String>,
    pub mode: Signal<String>,
    pub isolation: Signal<String>,
    pub socket: Signal<String>,
    pub kernel: Signal<String>,
    pub kernel_sha: Signal<String>,
    pub source: Signal<String>,
    pub rootfs: Signal<String>,
    pub rootfs_sha: Signal<String>,
    pub vcpus: Signal<String>,
    pub memory: Signal<String>,
    pub network: Signal<String>,
    pub address: Signal<String>,
    pub mac: Signal<String>,
    pub userdata: Signal<String>,
    pub boot_args: Signal<String>,
}
#[component]
pub fn Guided(fields: Fields) -> Element {
    let Fields {
        name,
        mut mode,
        isolation,
        socket,
        kernel,
        kernel_sha,
        mut source,
        rootfs,
        rootfs_sha,
        vcpus,
        memory,
        mut network,
        address,
        mac,
        userdata,
        boot_args,
    } = fields;
    let networks = use_resource(|| async { get("/v1/networks").await });
    rsx! {
        Field {
            label: "Name",
            id: "vm-name",
            value: name,
            required: true,
            placeholder: "build-runner",
        }
        fieldset {
            legend {
                "Runtime"
                Info { title: "Runtime ownership",
                    "Managed VMs use Firemage to prepare assets and start Firecracker. An external socket attaches to a Firecracker process you already manage."
                }
            }
            div { class: "radio-group",
                for (value, label) in [("managed", "Managed by Firemage"), ("socket", "Bring your own socket")] {
                    label {
                        input {
                            r#type: "radio",
                            name: "runtime-mode",
                            value,
                            checked: mode() == value,
                            onchange: move |_| mode.set(value.into()),
                        }
                        "{label}"
                    }
                }
            }
        }
        if mode() == "socket" {
            p { class: "small muted", "External process mode requires server opt-in. You are responsible for host isolation and process limits. Firemage cannot verify them." }
            Field {
                label: "Firecracker socket on host",
                id: "vm-socket",
                value: socket,
                required: true,
                placeholder: "/run/firecracker/vm.sock",
            }
        } else {
            firemage_webui_view_security::IsolationMode { value: isolation }
            Field {
                label: "Kernel path or HTTPS URL",
                id: "vm-kernel",
                value: kernel,
                required: true,
                placeholder: "/var/lib/firemage/assets/vmlinux",
            }
            if kernel().starts_with("https://") {
                Field {
                    label: "Kernel SHA-256",
                    id: "vm-kernel-sha",
                    value: kernel_sha,
                    required: true,
                }
            }
            fieldset {
                legend { "Root disk source" }
                div { class: "radio-group",
                    for (value, label) in [("local", "Local disk"), ("remote", "Remote disk"), ("oci", "OCI image")] {
                        label {
                            input {
                                r#type: "radio",
                                name: "asset-source",
                                value,
                                checked: source() == value,
                                onchange: move |_| source.set(value.into()),
                            }
                            "{label}"
                        }
                    }
                }
            }
            Field {
                label: match source().as_str() {
                    "remote" => "Root disk HTTPS URL",
                    "oci" => "OCI image reference",
                    _ => "Root disk path on host",
                },
                id: "vm-rootfs",
                value: rootfs,
                required: true,
            }
            if source() == "remote" {
                Field {
                    label: "Root disk SHA-256",
                    id: "vm-rootfs-sha",
                    value: rootfs_sha,
                    required: true,
                }
            }
        }
        div { class: "form-grid",
            Field {
                label: "vCPUs",
                id: "vm-cpus",
                value: vcpus,
                kind: "number",
                required: true,
            }
            Field {
                label: "Memory (MiB)",
                id: "vm-memory",
                value: memory,
                kind: "number",
                required: true,
            }
        }
        label { class: "field", r#for: "vm-network",
            span {
                "Network"
                Info { title: "Network access",
                    "No network creates a VM without a network interface. Isolated networks provide an interface with no external access, including for metadata. Firemage-only networks allow only the VM’s HTTP proxy and TCP tunnels. Configure destinations in the Egress tab after creating the VM. Host-only networks allow one explicit host IP. Guest addressing must be configured by your image or userdata."
                }
            }
            select {
                id: "vm-network",
                value: "{network}",
                onchange: move | e |
                                                        network.set(e.value()),
                option { value: "", "No network" }
                if let Some(Ok(rows)) = networks.read().as_ref() {
                    for row in rows.as_array().into_iter().flatten() {
                        option { value: r#"{row["name"].as_str().unwrap_or_default()}"#,
                            r#"{row["name"].as_str().unwrap_or_default()}"#
                        }
                    }
                }
            }
        }
        if !network().is_empty() {
            div { class: "form-grid",
                Field {
                    label: "Guest IPv4 address",
                    id: "vm-address",
                    value: address,
                    required: true,
                }
                Field {
                    label: "MAC address",
                    id: "vm-mac",
                    value: mac,
                    required: true,
                }
            }
        }
        Field { label: "Boot arguments", id: "vm-boot-args", value: boot_args }
        Editor {
            label: "Userdata (optional)",
            id: "vm-userdata",
            value: userdata,
            rows: 4,
        }
    }
}
