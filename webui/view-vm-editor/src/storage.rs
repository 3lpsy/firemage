use crate::fields::Fields;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;

#[component]
pub fn Storage(fields: Fields, existing: bool, onconfigure: EventHandler<String>) -> Element {
    let Fields {
        kernel,
        mut source,
        rootfs,
        rootfs_sha,
        rootfs_size,
        registry,
        ..
    } = fields;
    rsx! {
        if (fields.mode)() == "socket" {
            p { class: "small muted", "The external process manages its boot disks." }
        } else {
            crate::kernel::KernelPicker { selected: kernel }
            fieldset { legend { "Root disk source" },
                div { class: "radio-group",
                    for (value, label) in [("local", "Local disk"), ("remote", "Remote disk"), ("oci", "OCI image")] {
                        label { input { r#type: "radio", name: "asset-source", value, checked: source() == value,
                            onchange: move |_| source.set(value.into()) } "{label}" }
                    }
                }
            }
            Field { label: match source().as_str() { "remote" => "Root disk HTTPS URL", "oci" => "OCI image reference (tag or digest)", _ => "Root disk path on host" },
                id: "vm-rootfs", value: rootfs, required: true }
            if source() == "remote" { Field { label: "Root disk SHA-256", id: "vm-rootfs-sha", value: rootfs_sha, required: true } }
            if source() == "oci" {
                Field { label: "Root disk size (MiB)", id: "vm-rootfs-size", value: rootfs_size, kind: "number", required: true }
                details { class: "vm-form-registry", summary { "Registry access" }, crate::registry_fields::RegistryFields { value: registry } }
            }
            button { r#type: "button", onclick: move |_| onconfigure.call("storage".into()), "Configure initrd and additional drives" }
            if existing { p { class: "small muted", "Disks already prepared on the host cannot be replaced or removed. Create a new VM to change them." } }
        }
    }
}
