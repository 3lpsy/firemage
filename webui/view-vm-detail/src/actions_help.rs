use dioxus::prelude::*;
use firemage_webui_component_controls::Info;

#[component]
pub fn ActionsHelp() -> Element {
    rsx! {
        Info { title: "VM actions",
            dl { class: "key-values",
                dt { "Start" }
                dd { "Prepares as needed, then boots the guest." }
                dt { "Prepare" }
                dd { "Sets up disks and Firecracker without booting the guest." }
                dt { "Launch" }
                dd { "Starts an unconfigured Firecracker process for manual setup. Available only for eligible VMs." }
                dt { "Pause" }
                dd { "Freezes guest execution while keeping its memory and resources." }
                dt { "Resume" }
                dd { "Continues a paused guest from where it stopped." }
                dt { "Shut down" }
                dd { "Sends Ctrl+Alt+Del to request shutdown. Requires guest support." }
                dt { "Stop" }
                dd { "Force-stops Firecracker. Unsaved guest data may be lost." }
                dt { "Refresh" }
                dd { "Checks Firecracker and its process to update the displayed VM state." }
                dt { "Duplicate VM" }
                dd { "Copies configuration into a new VM, with fresh addresses if networked. Disks and outputs are not copied." }
                dt { "Delete VM" }
                dd { "Removes the VM, managed disks, and outputs. Snapshots are kept unless you choose to delete them too." }
                dt { "Save snapshot" }
                dd { "Saves a paused managed VM's state, memory, and disks. It stays paused." }
                dt { "Restore snapshot" }
                dd { "Replaces a compatible stopped VM's disks and restores saved state. Use Resume to continue." }
            }
        }
    }
}
