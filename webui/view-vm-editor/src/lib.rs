//! VM definition form with an equivalent full TOML editor.
mod fields;
mod registry;
mod registry_fields;
mod spec;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;
#[component]
pub fn VmEditor(
    #[props(default)] vm: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
) -> Element {
    let auth = use_auth();
    let existing = vm["id"].as_str().map(str::to_owned);
    let initial = vm["spec"].clone();
    let name = use_signal(|| text(&initial, "name"));
    let mode = use_signal(|| {
        if initial["socket"].is_string() {
            "socket".into()
        } else {
            "managed".into()
        }
    });
    let isolation = use_signal(|| {
        initial["security"]["mode"]
            .as_str()
            .unwrap_or("jailed")
            .to_owned()
    });
    let socket = use_signal(|| text(&initial, "socket"));
    let kernel = use_signal(|| {
        initial["kernel"]["path"]
            .as_str()
            .or(initial["kernel"]["url"].as_str())
            .unwrap_or_default()
            .to_owned()
    });
    let kernel_sha = use_signal(|| text(&initial["kernel"], "sha256"));
    let source = use_signal(|| {
        initial["rootfs"]["kind"]
            .as_str()
            .unwrap_or("local")
            .to_owned()
    });
    let rootfs = use_signal(|| {
        initial["rootfs"]["path"]
            .as_str()
            .or(initial["rootfs"]["url"].as_str())
            .or(initial["rootfs"]["image"].as_str())
            .unwrap_or_default()
            .to_owned()
    });
    let registry =
        use_signal(|| registry::RegistryForm::from_value(&initial["rootfs"]["registry"]));
    let rootfs_sha = use_signal(|| text(&initial["rootfs"], "sha256"));
    let vcpus = use_signal(|| initial["vcpus"].as_u64().unwrap_or(1).to_string());
    let memory = use_signal(|| initial["memory_mib"].as_u64().unwrap_or(256).to_string());
    let network = use_signal(|| text(&initial["network"], "network"));
    let address = use_signal(|| text(&initial["network"], "address"));
    let mac = use_signal(|| {
        initial["network"]["mac"]
            .as_str()
            .unwrap_or("06:00:ac:10:00:02")
            .to_owned()
    });
    let userdata = use_signal(|| text(&initial, "userdata"));
    let boot_args = use_signal(|| {
        initial["boot_args"]
            .as_str()
            .unwrap_or("console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw")
            .to_owned()
    });
    let mut advanced = use_signal(|| existing.is_some());
    let mut toml_text = use_signal(|| spec::to_toml(&initial).unwrap_or_default());
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let make_spec = move || {
        spec::Form {
            name: name(),
            mode: mode(),
            isolation: isolation(),
            socket: socket(),
            kernel: kernel(),
            kernel_sha: kernel_sha(),
            source: source(),
            rootfs: rootfs(),
            rootfs_sha: rootfs_sha(),
            registry: registry(),
            vcpus: vcpus(),
            memory: memory(),
            network: network(),
            address: address(),
            mac: mac(),
            userdata: userdata(),
            boot_args: boot_args(),
        }
        .spec()
    };
    let is_existing = existing.is_some();
    rsx! {
        Modal {
            title: if is_existing { "Configure virtual machine" } else { "Create virtual machine" },
            onclose,
            form {
                class: "modal-form",
                onsubmit: move |event| {
                    event.prevent_default();
                    if busy() {
                        return;
                    }
                    let spec = if advanced() {
                        toml::from_str::<Value>(&toml_text()).map_err(|e| e.to_string())
                    } else {
                        make_spec()
                    };
                    let spec = match spec {
                        Ok(s) => s,
                        Err(e) => {
                            error.set(e);
                            return;
                        }
                    };
                    let path = existing
                        .as_ref()
                        .map(|id| format!("/v1/vms/{id}"))
                        .unwrap_or("/v1/vms".into());
                    busy.set(true);
                    error.set(String::new());
                    spawn(async move {
                        match request(
                                if is_existing { "PUT" } else { "POST" },
                                &path,
                                Some(spec),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(_) => onsaved.call(()),
                            Err(e) => error.set(e),
                        }
                        busy.set(false);
                    });
                },
                div { class: "modal-form-body",
                    div { class: "tabs",
                        button {
                            r#type: "button",
                            class: if !advanced() { "active" } else { "" },
                            disabled: is_existing,
                            onclick: move |_| advanced.set(false),
                            "Guided setup"
                        }
                        button {
                            r#type: "button",
                            class: if advanced() { "active" } else { "" },
                            onclick: move |_| {
                                if !advanced() {
                                    match make_spec().and_then(|s| spec::to_toml(&s)) {
                                        Ok(s) => toml_text.set(s),
                                        Err(_) => {
                                            toml_text
                                                .set(
                                                    format!("name = {:?}\nvcpus = 1\nmemory_mib = 256\n", name()),
                                                )
                                        }
                                    }
                                }
                                advanced.set(true);
                            },
                            "Full TOML"
                        }
                    }
                    Notice { message: error() }
                    if advanced() {
                        Editor {
                            label: "VM configuration",
                            id: "vm-toml",
                            value: toml_text,
                            rows: 20,
                        }
                        p { class: "muted small",
                            "All VM settings are available here: assets, registry access, drives, network, userdata, boot files, metadata, host isolation, process limits, and external sockets. Isolation mode is fixed at creation."
                        }
                    } else {
                        fields::Guided {
                            fields: fields::Fields {
                                name,
                                mode,
                                isolation,
                                socket,
                                kernel,
                                kernel_sha,
                                source,
                                rootfs,
                                rootfs_sha,
                                registry,
                                vcpus,
                                memory,
                                network,
                                address,
                                mac,
                                userdata,
                                boot_args,
                            },
                        }
                    }
                }
                div { class: "actions end",
                    button { r#type: "button", onclick: move | _ | onclose
                                                        .call(()), "Cancel" }
                    button {
                        class: "primary",
                        r#type: "submit",
                        disabled: busy(),
                        if busy() {
                            "Saving…"
                        } else if is_existing {
                            "Save configuration"
                        } else {
                            "Create VM"
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
